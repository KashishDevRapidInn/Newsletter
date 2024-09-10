use crate::{
    db::PgPool,
    db_models::SubscriptionToken,
    schema::{
        subscription_tokens::{self, dsl as subs_token_dsl},
        subscriptions::{self, dsl as subs_dsl},
    },
};
use actix_web::{web, HttpResponse};
use diesel::prelude::*;
use serde::Deserialize;
use uuid::Uuid;

#[derive(Deserialize)]
pub struct Parameters {
    subscription_token: String,
}

#[tracing::instrument(name = "Confirm a pending subscriber", skip(parameters, pool))]
pub async fn confirm(parameters: web::Query<Parameters>, pool: web::Data<PgPool>) -> HttpResponse {
    let id: Option<Uuid> =
        match get_subscriber_id_from_token(&pool, parameters.subscription_token.clone()).await {
            Ok(id) => id,
            Err(_) => return HttpResponse::InternalServerError().finish(),
        };

    match id {
        None => HttpResponse::Unauthorized().finish(),
        Some(subscriber_id) => {
            if confirm_subscriber(&pool, subscriber_id).await.is_err() {
                return HttpResponse::InternalServerError().finish();
            }
            HttpResponse::Ok().finish()
        }
    }
}

#[tracing::instrument(name = "Mark subscriber as confirmed", skip(subscriber_id, pool))]
pub async fn confirm_subscriber(
    pool: &PgPool,
    subscriber_id: Uuid,
) -> Result<(), diesel::result::Error> {
    let pool = pool.clone();

    let result = web::block(move || {
        let mut conn = pool.get().expect("Couldn't get db connection from Pool");
        let result = diesel::update(subs_dsl::subscriptions.find(subscriber_id))
            .set(subs_dsl::status.eq(Some("confirmed".to_string())))
            .execute(&mut conn)?;
        Ok::<_, diesel::result::Error>(result)
    })
    .await;
    match result {
        Ok(Ok(_message)) => Ok(()),
        Ok(Err(err)) => return Err(err),
        Err(_err) => {
            return Err(diesel::result::Error::DatabaseError(
                diesel::result::DatabaseErrorKind::SerializationFailure,
                Box::new("The blocking operation was canceled.".to_string()),
            ))
        }
    }
}

#[tracing::instrument(name = "Get subscriber_id from token", skip(subscription_token, pool))]
pub async fn get_subscriber_id_from_token(
    pool: &PgPool,
    subscription_token: String,
) -> Result<Option<Uuid>, diesel::result::Error> {
    let pool = pool.clone();

    let result = web::block(move || {
        let mut conn = pool.get().expect("Couldn't get db connection from Pool");
        let result = subs_token_dsl::subscription_tokens
            .filter(subs_token_dsl::subscription_token.eq(subscription_token))
            .load::<SubscriptionToken>(&mut conn)?;
        Ok::<_, diesel::result::Error>(result)
    })
    .await;

    let subscriber_id = match result {
        Ok(Ok(tokens)) => tokens.into_iter().next().map(|token| token.subscriber_id),
        Ok(Err(err)) => return Err(err),
        Err(_err) => {
            return Err(diesel::result::Error::DatabaseError(
                diesel::result::DatabaseErrorKind::SerializationFailure,
                Box::new("The blocking operation was canceled.".to_string()),
            ))
        }
    };

    println!("user id is {:?}", subscriber_id);
    Ok(subscriber_id)
}
