mod db;

use newsletter::startup::run;
use std::net::TcpListener;
use db::establish_connection;

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let database_name = "newsletter";
    let pool= establish_connection(database_name);
    let listener = TcpListener::bind("127.0.0.1:8080")?;
    run(listener, pool)?.await
}