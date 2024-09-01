use diesel::prelude::*;
use diesel_migrations::{embed_migrations, EmbeddedMigrations};
pub const MIGRATIONS: EmbeddedMigrations = embed_migrations!("migrations");


use tokio;
use std::net::TcpListener;
use newsletter::db::{create_database};
use newsletter::db::PgPool;
use uuid::Uuid;
use dotenv::dotenv;
use std::env;
use diesel::r2d2::{ConnectionManager, Pool};
use diesel_migrations::MigrationHarness;
use newsletter::telemetry::{get_subscriber, init_subscriber};
use once_cell::sync::Lazy;
use newsletter::email_client::EmailClient;
use newsletter::domain::SubscriberEmail;
use secrecy::Secret;

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

pub fn spawn_app() -> TestApp {
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

    //new email_client
    dotenv().ok();
    let timeout = EmailClient::timeout();
    let sender = SubscriberEmail::parse(
                env::var("SENDER_EMAIL").expect("Failed to get sender email ")
            ).expect("Invalid sender email address");
    let email_client = EmailClient::new(
        env::var("BASE_URL").expect("Faield to get base url"),
        sender,
        Secret::new(env::var("AUTHORIZATION_TOKEN").expect("Failed to get auth token ")),
        timeout
    );

    let server = newsletter::startup::run(listener, pool.clone(),email_client).expect("Failed to bind address");
    let _ = tokio::spawn(server);

    TestApp {
        address,
        db_pool: pool,
        database_name,
    }
}