use diesel::prelude::*;
use diesel_migrations::{embed_migrations, EmbeddedMigrations};
pub const MIGRATIONS: EmbeddedMigrations = embed_migrations!("migrations");

use diesel::r2d2::{ConnectionManager, Pool};
use diesel_migrations::MigrationHarness;
use dotenv::dotenv;
use newsletter::db::create_database;
use newsletter::db::PgPool;
use newsletter::startup::Application;
use newsletter::telemetry::{get_subscriber, init_subscriber};
use once_cell::sync::Lazy;
use std::env;
use tokio;
use uuid::Uuid;
use wiremock::MockServer;

static TRACING: Lazy<()> = Lazy::new(|| {
    dotenv().ok();
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
    pub port: u16,
    pub address: String,
    pub db_pool: PgPool,
    pub database_name: String,
    pub email_server: MockServer,
}
pub struct ConfirmationLinks {
    pub html: reqwest::Url,
    pub plain_text: reqwest::Url,
}

impl TestApp {
    pub async fn post_subscriptions(&self, body: String) -> reqwest::Response {
        reqwest::Client::new()
            .post(&format!("{}/subscriptions", &self.address))
            .header("Content-Type", "application/x-www-form-urlencoded")
            .body(body)
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub fn get_confirmation_links(&self, email_request: &wiremock::Request) -> ConfirmationLinks {
        let body: serde_json::Value = serde_json::from_slice(&email_request.body).unwrap();

        let get_link = |s: &str| {
            let links: Vec<_> = linkify::LinkFinder::new()
                .links(s)
                .filter(|l| *l.kind() == linkify::LinkKind::Url)
                .collect();

            assert_eq!(links.len(), 1);
            let raw_link = links[0].as_str().to_owned();
            let mut confirmation_link = reqwest::Url::parse(&raw_link).unwrap();
            // Let's make sure we don't call random APIs on the web
            assert_eq!(confirmation_link.host_str().unwrap(), "127.0.0.1");
            confirmation_link.set_port(Some(self.port)).unwrap();
            confirmation_link
        };

        let html = get_link(&body["HtmlBody"].as_str().unwrap());
        let plain_text = get_link(&body["TextBody"].as_str().unwrap());

        ConfirmationLinks { html, plain_text }
    }
}

pub fn run_db_migrations(conn: &mut impl MigrationHarness<diesel::pg::Pg>) {
    conn.run_pending_migrations(MIGRATIONS)
        .expect("Could not run migrations");
}

pub async fn spawn_app() -> TestApp {
    // To Ensure that the tracing stack is only initialized once
    Lazy::force(&TRACING);
    let email_server = MockServer::start().await;
    let base_uri = email_server.uri();

    dotenv().ok();
    let database_name = Uuid::new_v4().to_string();
    let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    create_database(&database_name);

    let new_database_url = format!("{}/{}", database_url, database_name);
    let manager = ConnectionManager::<PgConnection>::new(new_database_url.clone());
    let pool = Pool::builder()
        .build(manager)
        .expect("Failed to create pool.");

    // Run migrations
    let mut conn = pool.get().expect("Couldn't get db connection from Pool");
    run_db_migrations(&mut conn);

    let application = Application::build(0, pool.clone(), Some(base_uri))
        .await
        .expect("Failed to build application");
    let application_port = application.port();
    let address = format!("http://127.0.0.1:{}", application_port);
    let _ = tokio::spawn(application.run_until_stopped());

    TestApp {
        port: application_port,
        address,
        db_pool: pool.clone(),
        database_name,
        email_server,
    }
}
