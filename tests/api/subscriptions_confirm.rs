use crate::helpers::spawn_app;
use diesel::prelude::*;
use newsletter::db::drop_database;
use newsletter::db_models::Subscription;
use newsletter::schema::subscriptions::dsl::*;
use reqwest::{self, Url};
use wiremock::matchers::{method, path};
use wiremock::{Mock, ResponseTemplate};

#[tokio::test]
async fn confirmations_without_token_are_rejected_with_a_400() {
    let app = spawn_app().await;

    let response = reqwest::get(&format!("{}/subscriptions/confirm", app.address))
        .await
        .unwrap();

    assert_eq!(response.status().as_u16(), 400);
    drop_database(&app.database_name);
}

#[tokio::test]
async fn the_link_returned_by_subscribe_returns_a_200_if_called() {
    let app = spawn_app().await;
    let body = "name=kk%20kashyap&email=kk%40gmail.com";

    Mock::given(path("/email"))
        .and(method("POST"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&app.email_server)
        .await;

    app.post_subscriptions(body.into()).await;

    let email_request = &app.email_server.received_requests().await.unwrap()[0];
    let confirmation_links = app.get_confirmation_links(&email_request);

    let response = reqwest::get(confirmation_links.html).await.unwrap();

    assert_eq!(response.status().as_u16(), 200);
    drop_database(&app.database_name);
}
#[tokio::test]
async fn clicking_on_the_confirmation_link_confirms_a_subscriber() {
    let app = spawn_app().await;
    let body = "name=kk%20kashyap&email=kashishh%40gmail.com";

    Mock::given(path("/email"))
        .and(method("POST"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&app.email_server)
        .await;

    app.post_subscriptions(body.into()).await;

    let email_request = &app.email_server.received_requests().await.unwrap()[0];
    let confirmation_links = app.get_confirmation_links(&email_request);

    reqwest::get(confirmation_links.html)
        .await
        .unwrap()
        .error_for_status()
        .unwrap();

    let mut conn = app
        .db_pool
        .get()
        .expect("Couldn't get db connection from Pool");

    let inserted_subscription = subscriptions
        .order(subscribed_at.desc())
        .limit(1)
        .load::<Subscription>(&mut conn)
        .expect("Failed to fetch saved subscription");

    let inserted_subscription = &inserted_subscription[0];
    assert_eq!(inserted_subscription.email, "kashishh@gmail.com");
    assert_eq!(inserted_subscription.name, "kk kashyap");
    assert_eq!(inserted_subscription.status, Some("confirmed".to_string()));

    drop_database(&app.database_name);
}
