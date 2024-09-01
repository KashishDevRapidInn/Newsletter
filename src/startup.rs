use std::net::TcpListener;
use crate::db::establish_connection;
use crate::email_client::EmailClient;
use crate::domain::SubscriberEmail;
use secrecy::Secret;
use actix_web::{web, App, HttpServer};
use actix_web::dev::Server;
// use actix_web::middleware::Logger;
use tracing_actix_web::TracingLogger;


use crate::db::PgPool;


use crate::routes::{
    health_check::health_check,
    subscriptions::subscribe,
};

pub async fn build()-> Result<Server, std::io::Error>{
    use dotenv::dotenv;
    use std::env;
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
    
    let database_name = "newsletter";
    let pool= establish_connection(database_name);
    let listener = TcpListener::bind("127.0.0.1:8080")?;
    run(listener, pool, email_client)
}

// We need to mark `run` as public,  It is no longer a binary entrypoint, therefore we can mark it as async, without having to use any proc-macro incantation.
pub fn run(listener: TcpListener, db_pool: PgPool, email_client: EmailClient)-> Result<Server, std::io::Error> {

    let db_pool = web::Data::new(db_pool);
    let email_client = web::Data::new(email_client);

    let server = HttpServer::new(move|| {
    App::new()
        .wrap(TracingLogger::default())
        .app_data(db_pool.clone())
        .app_data(email_client.clone())
        .route("/health_check", web::get().to(health_check))
        .route("/subscriptions", web::post().to(subscribe))
    })
    .listen(listener)?
    .run();
    // No .await here
    Ok(server)
}
