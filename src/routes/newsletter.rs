use crate::{
    db::PgPool,
    domain::SubscriberEmail,
    email_client::EmailClient,
    routes::subscriptions::error_chain_fmt,
    schema::subscriptions::{self, dsl::*},
};
use actix_web::{http::StatusCode, web, HttpResponse, ResponseError};
use anyhow::Context;
use diesel::prelude::*;
use diesel::Queryable;
use serde::Deserialize;
use thiserror::Error;
use tracing::subscriber;

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
    #[error(transparent)]
    UnexpectedError(#[from] anyhow::Error),
}

impl std::fmt::Debug for PublishError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}
impl ResponseError for PublishError {
    fn status_code(&self) -> actix_web::http::StatusCode {
        match self {
            PublishError::UnexpectedError(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
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

pub async fn publish_newsletter(
    body: web::Json<BodyData>,
    pool: web::Data<PgPool>,
    email_client: web::Data<EmailClient>,
) -> Result<HttpResponse, PublishError> {
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
