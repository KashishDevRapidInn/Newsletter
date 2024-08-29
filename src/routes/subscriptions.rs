use actix_web::{web, HttpResponse};
use diesel::prelude::*;
use chrono::DateTime;
use chrono::Utc;
use diesel::prelude::Insertable;
use uuid::Uuid;
use crate::db::PgPool;
use serde::Deserialize;
use crate::schema::subscriptions::{self, dsl::*};
use tracing::{info, error, info_span, Instrument};
use crate::domain::{NewSubscriber, SubscriberName, SubscriberEmail};


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
}
#[tracing::instrument(
    name = "Saving new subscriber details in the database",
    skip(pool, newSubscriber)
)]

fn insert_subscriber(pool: &PgPool, newSubscriber: &NewSubscriber) -> Result<(), diesel::result::Error> {
    let mut conn = pool.get().expect("Couldn't get db connection from Pool");

    let new_subscription = NewSubscription {
        id: Uuid::new_v4(),
        email: newSubscriber.email.as_ref().to_string(),
        name: newSubscriber.name.as_ref().to_string(), // to convert as type to reference of another type
        subscribed_at: Utc::now().naive_utc(),  
    };

    diesel::insert_into(subscriptions::table)
        .values(&new_subscription)
        .execute(&mut conn)?;
    Ok(())
}
#[tracing::instrument(
    name = "Adding a new subscriber",
    skip(form, pool),
    fields(
        subscriber_email = %form.email,
        subscriber_name = %form.name
    )
)]
pub async fn subscribe(form: web::Form<FormData>, pool: web::Data<PgPool>)-> HttpResponse{

    let new_subscriber = match form.0.try_into() {
        Ok(form) => form,
        Err(_) => return HttpResponse::BadRequest().finish(),
    };

    let result = web::block(move || {
        insert_subscriber(&pool, &new_subscriber)
    })
    .instrument(info_span!("Saving new subscriber details in the database"))
    .await;

    match result {
        Ok(Ok(())) =>{
            HttpResponse::Ok().finish()
        }

        Ok(Err(err)) => {
            error!("Failed to execute query: {:?}", err);
            HttpResponse::InternalServerError().body(format!("Error: {:?}", err))
        }
        Err(err) => HttpResponse::InternalServerError().body(format!("Blocking error: {:?}", err)),
    }
}

impl TryFrom<FormData> for NewSubscriber {
    type Error = String;
    fn try_from(value: FormData) -> Result<Self, Self::Error> {
        let valid_name = SubscriberName::parse(value.name)?;
        let valid_email = SubscriberEmail::parse(value.email)?;
        Ok(Self { email:valid_email, name: valid_name })
    }
}
