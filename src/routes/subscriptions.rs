use crate::db::PgPool;
use crate::domain::{NewSubscriber, SubscriberEmail, SubscriberName};
use crate::email_client::EmailClient;
use crate::schema::subscription_tokens;
use crate::schema::subscription_tokens::dsl as subs_token_dsl;
use crate::schema::subscriptions::{self, dsl::*};
use crate::startup::ApplicationBaseUrl;
use actix_web::{web, HttpResponse, ResponseError};
use chrono::Utc;
use diesel::pg::PgConnection;
use diesel::prelude::Insertable;
use diesel::prelude::*;
use rand::distributions::Alphanumeric;
use rand::{thread_rng, Rng};
use reqwest;
use serde::Deserialize;
use tracing;
use uuid::Uuid;

#[derive(Deserialize)]
pub struct FormData {
    email: String,
    name: String,
}
#[derive(Insertable)]
#[table_name = "subscriptions"]
pub struct NewSubscription {
    id: Uuid,
    pub email: String,
    pub name: String,
    pub subscribed_at: chrono::NaiveDateTime,
    pub status: String,
}

pub struct StoreTokenError(diesel::result::Error);
impl std::fmt::Display for StoreTokenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "A database error was encountered while trying to store a subscription token."
        )
    }
}

impl std::error::Error for StoreTokenError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.0)
    }
}

impl std::fmt::Debug for StoreTokenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

fn error_chain_fmt(
    e: &impl std::error::Error,
    f: &mut std::fmt::Formatter<'_>,
) -> std::fmt::Result {
    writeln!(f, "{}\n", e)?;
    let mut current = e.source();
    while let Some(cause) = current {
        writeln!(f, "Caused by:\n\t{}", cause)?;
        current = cause.source();
    }
    Ok(())
}

fn generate_subscription_token() -> String {
    let mut rng = thread_rng();
    std::iter::repeat_with(|| rng.sample(Alphanumeric))
        .map(char::from)
        .take(25)
        .collect()
}

#[tracing::instrument(
    name = "Saving new subscriber details in the database",
    skip(conn, new_subscriber)
)]

fn insert_subscriber(
    conn: &mut PgConnection,
    new_subscriber: &NewSubscriber,
) -> Result<Uuid, diesel::result::Error> {
    let subscriber_id = Uuid::new_v4();

    let new_subscription = NewSubscription {
        id: subscriber_id,
        email: new_subscriber.email.as_ref().to_string(),
        name: new_subscriber.name.as_ref().to_string(), // to convert as type to reference of another type
        subscribed_at: Utc::now().naive_utc(),
        status: "pending_confirmation".to_string(),
    };

    diesel::insert_into(subscriptions::table)
        .values(&new_subscription)
        .execute(conn)?;
    Ok(subscriber_id)
}
#[tracing::instrument(
    name = "Send a confirmation email to a new subscriber",
    skip(email_client, new_subscriber)
)]
async fn send_confirmation_email(
    email_client: &EmailClient,
    new_subscriber: NewSubscriber,
    application_base_url: &str,
    subscription_token: &str,
) -> Result<(), reqwest::Error> {
    let confirmation_link = format!(
        "{}/subscriptions/confirm?subscription_token={}",
        application_base_url, subscription_token
    );

    let plain_body = format!(
        "Welcome to our newsletter!\nVisit {} to confirm your subscription.",
        confirmation_link
    );
    let html_body = format!(
        "Welcome to our newsletter!<br />\
        Click <a href=\"{}\">here</a> to confirm your subscription.",
        confirmation_link
    );

    email_client
        .send_email(new_subscriber.email, "Welcome!", &html_body, &plain_body)
        .await
}

#[tracing::instrument(
    name = "Adding a new subscriber",
    skip(form, pool, email_client, application_base_url),
    fields(
        subscriber_email = %form.email,
        subscriber_name = %form.name,
    )
)]
pub async fn subscribe(
    form: web::Form<FormData>,
    pool: web::Data<PgPool>,
    email_client: web::Data<EmailClient>,
    application_base_url: web::Data<ApplicationBaseUrl>,
) -> Result<HttpResponse, actix_web::Error> {
    let new_subscriber = match form.0.try_into() {
        Ok(form) => form,
        Err(_) => return Err(actix_web::error::ErrorBadRequest("Invalid form data")),
    };
    let mut conn = pool.get().expect("failed to get db connection from pool");

    let token = conn
        .transaction::<_, diesel::result::Error, _>(|mut conn| {
            let subscriber_id = insert_subscriber(&mut conn, &new_subscriber)
                .map_err(|_| diesel::result::Error::RollbackTransaction)?;

            let subscription_token = generate_subscription_token();

            store_token(&mut conn, &subscriber_id, &subscription_token).map_err(|e| {
                tracing::error!("Failed to store token: {}", e);
                diesel::result::Error::RollbackTransaction
            })?;
            Ok(subscription_token)
        })
        .map_err(|e| {
            tracing::error!("Database transaction error: {}", e);
            actix_web::error::ErrorInternalServerError("Transaction error")
        })?;

    let subscription_token = token;

    if send_confirmation_email(
        &email_client,
        new_subscriber,
        &application_base_url.0,
        &subscription_token,
    )
    .await
    .is_err()
    {
        return Err(actix_web::error::ErrorInternalServerError(
            "Failed to send confirmation email",
        ));
    }
    Ok(HttpResponse::Ok().finish())
}

#[tracing::instrument(
    name = "Store subscription token in the database",
    skip(subscription_token, conn)
)]
pub fn store_token(
    conn: &mut PgConnection,
    subscriber_id: &Uuid,
    subscription_token: &str,
) -> Result<(), StoreTokenError> {
    diesel::insert_into(subscription_tokens::table)
        .values((
            subs_token_dsl::subscriber_id.eq(subscriber_id),
            subs_token_dsl::subscription_token.eq(subscription_token),
        ))
        .execute(conn)
        .map_err(StoreTokenError)?;
    Ok(())
}

impl TryFrom<FormData> for NewSubscriber {
    type Error = String;
    fn try_from(value: FormData) -> Result<Self, Self::Error> {
        let valid_name = SubscriberName::parse(value.name)?;
        let valid_email = SubscriberEmail::parse(value.email)?;
        Ok(Self {
            email: valid_email,
            name: valid_name,
        })
    }
}
