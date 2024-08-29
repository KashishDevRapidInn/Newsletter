use diesel::prelude::*;
use diesel_migrations::{embed_migrations, EmbeddedMigrations};
pub const MIGRATIONS: EmbeddedMigrations = embed_migrations!("migrations");

use reqwest::Client;
use tokio;
use std::net::TcpListener;
use newsletter::db::{establish_connection, create_database, drop_database};
use newsletter::schema::subscriptions::dsl::*;
use newsletter::db_models::Subscription;
use newsletter::db::PgPool;
use uuid::Uuid;
use dotenv::dotenv;
use std::env;
use diesel::r2d2::{ConnectionManager, Pool};
use diesel_migrations::MigrationHarness;
use newsletter::telemetry::{get_subscriber, init_subscriber};
use once_cell::sync::Lazy;

// This static variable will ensure `init_subscriber` is called only once
static TRACING: Lazy<()> = Lazy::new(|| {
    let default_filter_level = "info".to_string();
    let subscriber_name = "test".to_string();
    // We cannot assign the output of `get_subscriber` to a variable based on the value of `TEST_LOG`
    // because the sink is part of the type returned by `get_subscriber`, therefore they are not the
    // same type. We could work around it, but this is the most straight-forward way of moving forward.
    if std::env::var("TEST_LOG").is_ok() {
    let subscriber = get_subscriber(subscriber_name, default_filter_level, std::io::stdout);
    init_subscriber(subscriber);
    } else {
    let subscriber = get_subscriber(subscriber_name, default_filter_level, std::io::sink);
    init_subscriber(subscriber);
    };
});


pub struct TestApp {
    pub address: String,
    pub db_pool: PgPool,
    pub database_name: String,
}

pub fn run_db_migrations(conn: &mut impl MigrationHarness<diesel::pg::Pg>) {
    conn.run_pending_migrations(MIGRATIONS).expect("Could not run migrations");
}

fn spawn_app() -> TestApp {
    // To Ensure that the tracing stack is only initialized once
    Lazy::force(&TRACING);

    dotenv().ok();
    let database_name = Uuid::new_v4().to_string();
    let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    create_database(&database_name);

    let new_database_url = format!("{}/{}", database_url, database_name);
    let manager = ConnectionManager::<PgConnection>::new(new_database_url.clone());
    let pool = Pool::builder().build(manager).expect("Failed to create pool.");

    // Run migrations
    let mut conn = pool.get().expect("Couldn't get db connection from Pool");
    run_db_migrations(&mut conn);

    let listener = TcpListener::bind("127.0.0.1:0").expect("Failed to bind random port");

    let port = listener.local_addr().unwrap().port();
    let address = format!("http://127.0.0.1:{}", port);

    let server = newsletter::startup::run(listener, pool.clone()).expect("Failed to bind address");
    let _ = tokio::spawn(server);

    TestApp {
        address,
        db_pool: pool,
        database_name,
    }
}


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