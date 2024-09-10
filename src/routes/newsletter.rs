use crate::{
    db::PgPool,
    domain::SubscriberEmail,
    email_client::EmailClient,
    routes::subscriptions::error_chain_fmt,
    schema::{
        subscriptions::{self, dsl::*},
        users,
    },
};
use actix_web::http::header::{self, HeaderMap, HeaderValue};
use actix_web::{http::StatusCode, web, HttpRequest, HttpResponse, ResponseError};
use anyhow::Context;
use base64;
use diesel::prelude::*;
use diesel::Queryable;
use secrecy::{ExposeSecret, Secret};
use serde::Deserialize;
use sha3::Digest;
use uuid::Uuid;

#[derive(Deserialize)]
pub struct BodyData {
    title: String,
    content: Content,
}
#[derive(Deserialize)]
pub struct Content {
    html: String,
    text: String,
}
#[derive(Debug, Deserialize, Queryable)]
pub struct ConfirmedSubscriber {
    email: SubscriberEmail,
}

#[derive(thiserror::Error)]
pub enum PublishError {
    #[error("Authentication failed.")]
    AuthError(#[source] anyhow::Error),
    #[error(transparent)]
    UnexpectedError(#[from] anyhow::Error),
}

impl std::fmt::Debug for PublishError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}
impl ResponseError for PublishError {
    fn error_response(&self) -> HttpResponse {
        match self {
            PublishError::UnexpectedError(_) => {
                HttpResponse::new(StatusCode::INTERNAL_SERVER_ERROR)
            }
            PublishError::AuthError(_) => {
                let mut response = HttpResponse::new(StatusCode::UNAUTHORIZED);
                let header_value = HeaderValue::from_str(r#"Basic realm="publish""#).unwrap();
                response
                    .headers_mut()
                    // actix_web::http::header provides a collection of constants
                    // for the names of several well-known/standard HTTP headers
                    .insert(header::WWW_AUTHENTICATE, header_value);
                response
            }
        }
    }
}
struct Credentials {
    username: String,
    password: Secret<String>,
}
fn basic_authentication(headers: &HeaderMap) -> Result<Credentials, anyhow::Error> {
    // The header value, if present, must be a valid UTF8 string
    let header_value = headers
        .get("Authorization")
        .context("The 'Authorization' header was missing")?
        .to_str()
        .context("The 'Authorization' header was not a valid UTF8 string.")?;

    let base64encoded_segment = header_value
        .strip_prefix("Basic ")
        .context("The authorization scheme was not 'Basic'.")?;

    let decoded_bytes = base64::decode_config(base64encoded_segment, base64::STANDARD)
        .context("Failed to base64-decode 'Basic' credentials.")?;

    let decoded_credentials = String::from_utf8(decoded_bytes)
        .context("The decoded credential string is not valid UTF8.")?;

    // Split into two segments, using ':' as delimitator
    let mut credentials = decoded_credentials.splitn(2, ':');
    let username = credentials
        .next()
        .ok_or_else(|| anyhow::anyhow!("A username must be provided in 'Basic' auth."))?
        .to_string();
    let password = credentials
        .next()
        .ok_or_else(|| anyhow::anyhow!("A password must be provided in 'Basic' auth."))?
        .to_string();
    Ok(Credentials {
        username,
        password: Secret::new(password),
    })
}

async fn validate_credentials(
    credentials: Credentials,
    pool: &PgPool,
) -> Result<uuid::Uuid, PublishError> {
    let password_hash = sha3::Sha3_256::digest(credentials.password.expose_secret().as_bytes());
    let password_hash = format!("{:x}", password_hash);
    let mut conn = pool.get().expect("Failed to get db connection from pool");

    let user_id = users::table
        .filter(users::username.eq(credentials.username))
        .filter(users::password_hash.eq(password_hash))
        .select(users::user_id)
        .load::<Uuid>(&mut conn)
        .optional()
        .map_err(|err| PublishError::UnexpectedError(anyhow::anyhow!(err)))?;

    match user_id.and_then(|ids| ids.into_iter().next()) {
        //since .load returns a vector and_then is used to handle the Option returned by optional(), and into_iter().next() gets the first item from the vector.
        Some(id_user) => Ok(id_user),
        None => Err(PublishError::AuthError(anyhow::anyhow!(
            "Invalid username or password."
        ))),
    }
}

#[tracing::instrument(name = "Get confirmed subscribers", skip(pool))]
async fn get_confirmed_subscribers(
    pool: &web::Data<PgPool>,
) -> Result<Vec<Result<ConfirmedSubscriber, anyhow::Error>>, anyhow::Error> {
    let mut conn = pool.get().expect("Failed to get db connection from pool");

    let rows = subscriptions::table
        .filter(subscriptions::status.eq("confirmed"))
        .select(subscriptions::email)
        .load::<String>(&mut conn)?;
    let confirmed_subscribers = rows
        .into_iter()
        .map(|r| match SubscriberEmail::parse(r) {
            Ok(user_email) => Ok(ConfirmedSubscriber { email: user_email }),
            Err(error) => Err(anyhow::anyhow!(error)),
        })
        .collect();

    Ok(confirmed_subscribers)
}
#[tracing::instrument(
name = "Publish a newsletter issue",
skip(body, pool, email_client, request),
fields(username=tracing::field::Empty, user_id=tracing::field::Empty) // Defines fields to be included in the span. Here, username and user_id are included but are initially empty. These fields will be populated later in the function.
)]

pub async fn publish_newsletter(
    body: web::Json<BodyData>,
    pool: web::Data<PgPool>,
    email_client: web::Data<EmailClient>,
    request: HttpRequest,
) -> Result<HttpResponse, PublishError> {
    let credentials = basic_authentication(request.headers()).map_err(PublishError::AuthError)?;
    tracing::Span::current().record("username", &tracing::field::display(&credentials.username));

    let user_id = validate_credentials(credentials, &pool).await?;
    tracing::Span::current().record("user_id", &tracing::field::display(&user_id));

    let subscribers = get_confirmed_subscribers(&pool).await?;

    for subscriber in subscribers {
        match subscriber {
            Ok(subscriber) => {
                email_client
                    .send_email(
                        &subscriber.email,
                        &body.title,
                        &body.content.html,
                        &body.content.text,
                    )
                    .await
                    // with_context is used to convert the error variant of Result into anyhow::Error while enriching it with contextual information.
                    // with_context is lazy i.e., only called in case of an error
                    .with_context(|| {
                        format!("Failed to send newsletter issue to {}", subscriber.email)
                    })?;
            }
            Err(error) => {
                tracing::warn!(
                // We record the error chain as a structured fieldon the log record.
                error.cause_chain = ?error,
                // Using `\` to split a long string literal overtwo lines, without creating a `\n` character.
                "Skipping a confirmed subscriber. \
                Their stored contact details are invalid",
                );
            }
        }
    }

    Ok(HttpResponse::Ok().finish())
}
