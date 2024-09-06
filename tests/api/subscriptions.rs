use crate::helpers::spawn_app;
use diesel::prelude::*;
use newsletter::db::drop_database;
use newsletter::db_models::Subscription;
use newsletter::schema::subscriptions::dsl::*;
use wiremock::matchers::{method, path};
use wiremock::{Mock, ResponseTemplate};

#[tokio::test]
async fn subscribe_returns_a_200_for_valid_form_data() {
    let app = spawn_app().await;

    let body = "name=kk%20kashyap&email=kashishh%40gmail.com";
    Mock::given(path("/email"))
        .and(method("POST"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&app.email_server)
        .await;

    let response = app.post_subscriptions(body.into()).await;
    assert_eq!(200, response.status().as_u16());

    drop_database(&app.database_name);
}

#[tokio::test]
async fn subscribe_persists_the_new_subscriber() {
    let app = spawn_app().await;

    let body = "name=kk%20kashyap&email=kashishh%40gmail.com";
    Mock::given(path("/email"))
        .and(method("POST"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&app.email_server)
        .await;

    let response = app.post_subscriptions(body.into()).await;
    assert_eq!(200, response.status().as_u16());

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
    assert_eq!(
        inserted_subscription.status,
        Some("pending_confirmation".to_string())
    );

    drop_database(&app.database_name);
}

#[tokio::test]
async fn subscribe_returns_a_400_when_data_is_missing() {
    let app = spawn_app().await;
    let test_cases = vec![
        ("name=kk%20kashyap", "missing the email"),
        ("email=kk%40gmail.com", "missing the name"),
        ("", "missing both name and email"),
    ];
    for (invalid_body, error_message) in test_cases {
        let response = app.post_subscriptions(invalid_body.into()).await;

        assert_eq!(
            400,
            response.status().as_u16(),
            // Additional customised error message on test failure
            "The API did not fail with 400 Bad Request when the payload was {}.",
            error_message
        );
    }
    drop_database(&app.database_name);
}

#[tokio::test]
async fn subscribe_returns_a_400_when_fields_are_present_but_invalid() {
    let app = spawn_app().await;
    let test_cases = vec![
        ("name=&email=kk%40gmail.com", "empty name"),
        ("name=kk&email=", "empty email"),
        ("name=kk&email=definitely-not-an-email", "invalid email"),
    ];

    for (body, description) in test_cases {
        let response = app.post_subscriptions(body.into()).await;
        assert_eq!(
            400,
            response.status().as_u16(),
            "The API did not return a 400 Bad Request when the payload was {}.",
            description
        );
    }
    drop_database(&app.database_name);
}

#[tokio::test]
async fn subscribe_sends_a_confirmation_email_for_valid_data() {
    let app = spawn_app().await;
    let body = "name=le%20guin&email=ursula_le_guin%40gmail.com";
    Mock::given(path("/email"))
        .and(method("POST"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&app.email_server)
        .await;

    app.post_subscriptions(body.into()).await;

    drop_database(&app.database_name);
}

#[tokio::test]
async fn subscribe_sends_a_confirmation_email_with_a_link() {
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

    assert_eq!(confirmation_links.html, confirmation_links.plain_text);
    drop_database(&app.database_name);
}

#[tokio::test]
async fn subscribe_fails_if_there_is_a_fatal_database_error() {
    let app = spawn_app().await;
    let body = "name=le%20guin&email=ursula_le_guin%40gmail.com";
    let mut conn = app
        .db_pool
        .get()
        .expect("Couldn't get db connection from Pool");
    // Sabotage the database
    diesel::sql_query("ALTER TABLE subscription_tokens DROP COLUMN subscription_token;")
        .execute(&mut conn)
        .expect("Failed to execute sabotage query");

    let response = app.post_subscriptions(body.into()).await;

    assert_eq!(response.status().as_u16(), 500);
    drop_database(&app.database_name);
}
