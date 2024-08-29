mod db;
mod telemetry;

use newsletter::startup::run;
use std::net::TcpListener;
use db::establish_connection;
use telemetry::{get_subscriber, init_subscriber};

#[tokio::main]
async fn main() -> std::io::Result<()> {

    let subscriber = get_subscriber("newsletter_kk".into(), "info".into(), std::io::stdout);
    init_subscriber(subscriber);

    let database_name = "newsletter";
    let pool= establish_connection(database_name);
    let listener = TcpListener::bind("127.0.0.1:8080")?;
    run(listener, pool)?.await
}