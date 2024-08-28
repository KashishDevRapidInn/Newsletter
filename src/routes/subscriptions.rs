use actix_web::{web, App, HttpResponse, HttpServer};
use diesel::prelude::*;
use actix_web::dev::Server;
use chrono::DateTime;
use chrono::Utc;
use diesel::prelude::Insertable;
use uuid::Uuid;
use crate::db::{establish_connection, PgPool};
use std::net::TcpListener;
use serde::Deserialize;
use crate::schema::subscriptions::{self, dsl::*};


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

pub async fn subscribe(form: web::Form<FormData>, pool: web::Data<PgPool>) -> HttpResponse {
    let pool = pool.clone();

    let result = web::block(move || {
        let mut conn = pool.get().expect("Couldn't get db connection from Pool");

         let new_subscription = NewSubscription {
            id: Uuid::new_v4(),
            email: form.email.clone(),
            name: form.name.clone(),
            subscribed_at: Utc::now().naive_utc(),
        };

        diesel::insert_into(subscriptions::table)
            .values(&new_subscription)
            .execute(&mut conn)?;  // was getting error on this because  to sql query diesel is not applicable to chrono so to enable it added chrono in features in cargo.toml

        Ok::<_, diesel::result::Error>("Subscription created".to_string())
    })
    .await;

    match result {
        Ok(Ok(message)) => HttpResponse::Ok().finish(),
        Ok(Err(err)) => HttpResponse::InternalServerError().body(format!("Error: {:?}", err)),
        Err(err) => HttpResponse::InternalServerError().body(format!("Blocking error: {:?}", err)),
    }
}
