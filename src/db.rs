use diesel::pg::PgConnection;
use diesel::r2d2::{ConnectionManager, Pool};
use dotenv::dotenv;
use diesel::prelude::*;
use std::env;
use diesel::sql_query;
use secrecy::{Secret, ExposeSecret};
// use diesel::r2d2::PoolError; 
pub type PgPool = Pool<ConnectionManager<PgConnection>>;

pub fn establish_connection(database_name: &str) -> PgPool {
    dotenv().ok();
    let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let secret_url = Secret::new(format!("{}/{}", database_url, database_name));
    let new_database_url = secret_url.expose_secret();

    let manager = ConnectionManager::<PgConnection>::new(new_database_url);
    Pool::builder().build(manager).expect("Failed to create pool.")
}

pub fn create_database(database_name: &str) {
   dotenv().ok();
    let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let secret_url = Secret::new(database_url);
    let connection_url = secret_url.expose_secret();

    let mut connection = PgConnection::establish(&connection_url)
        .expect("Failed to connect to Postgres");

    let create_db_query = format!(r#"CREATE DATABASE "{}";"#, database_name);
    sql_query(&create_db_query)
        .execute(&mut connection)
        .expect("Failed to create database");
    println!("Database '{}' created", database_name);
}

pub fn drop_database(database_name: &str) {
    dotenv().ok();

    let default_db_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
     let secret_url = Secret::new(default_db_url);
    let connection_url = secret_url.expose_secret();

    // Here I'm connecting to Postgres 
    let mut connection = PgConnection::establish(&connection_url)
        .expect("Failed to connect to the maintenance database");

    // My drop db logic wasn't working because I was trying to drop db which had active connection, so i need ti dekete my active connections
    let terminate_query = format!(r#"
        SELECT pg_terminate_backend(pid) 
        FROM pg_stat_activity 
        WHERE datname = '{}';
    "#, database_name);

    if let Err(e) = sql_query(&terminate_query).execute(&mut connection) {
        eprintln!("Failed to terminate connections: {}", e);
        return;
    }

    // Dropping db
    let drop_query = format!(r#"DROP DATABASE IF EXISTS "{}";"#, database_name);

    if let Err(e) = sql_query(&drop_query).execute(&mut connection) {
        eprintln!("Failed to drop database: {}", e);
    } else {
        println!("Database '{}' dropped successfully.", database_name);
    }
}