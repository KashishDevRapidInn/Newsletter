use crate::helpers::spawn_app;
use newsletter::db::{drop_database};
use reqwest::Client;
use diesel::prelude::*;
use newsletter::schema::subscriptions::dsl::*;
use newsletter::db_models::Subscription;

#[tokio::test]
async fn health_check_works() {
    
    let app = spawn_app();
    let client = Client::new();
    let response = client
        .get(&format!("{}/health_check", &app.address))
        .send()
        .await
        .expect("Failed to execute request.");

    assert!(response.status().is_success());
    assert_eq!(Some(0), response.content_length());
    drop_database(&app.database_name);
}

#[tokio::test]
async fn subscribe_returns_a_200_for_valid_form_data(){

    
    let app= spawn_app();
    let client= Client::new();
    let body= "name=kk%20kashyap&email=kashishh%40gmail.com";

    let response= client
                    .post(&format!("{}/subscriptions", &app.address))
                    .header("Content-Type", "application/x-www-form-urlencoded")
                    .body(body)
                    .send()
                    .await
                    .expect("Failed to execute request");
    assert_eq!(200, response.status().as_u16());

    let mut conn = app.db_pool.get().expect("Couldn't get db connection from Pool");

    let inserted_subscription= subscriptions
                                .order(subscribed_at.desc())
                                .limit(1)
                                .load::<Subscription>(&mut conn)
                                .expect("Failed to fetch saved subscription");

    let inserted_subscription = &inserted_subscription[0];
    assert_eq!(inserted_subscription.email, "kashishh@gmail.com");
    assert_eq!(inserted_subscription.name, "kk kashyap");

    drop_database(&app.database_name);
}

#[tokio::test]
async fn subscribe_returns_a_400_when_data_is_missing() {
    let app = spawn_app();
    let client = Client::new();
    let test_cases = vec![
                        ("name=kk%20kashyap", "missing the email"),
                        ("email=kk%40gmail.com", "missing the name"),
                        ("", "missing both name and email")
                        ];
    for (invalid_body, error_message) in test_cases {

        let response = client
        .post(&format!("{}/subscriptions", &app.address))
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body(invalid_body)
        .send()
        .await
        .expect("Failed to execute request.");

        assert_eq!(400,
            response.status().as_u16(),
            // Additional customised error message on test failure
            "The API did not fail with 400 Bad Request when the payload was {}.",
            error_message
        );
    }
     drop_database(&app.database_name);
}