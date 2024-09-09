use crate::domain::SubscriberEmail;
use crate::email_client::EmailClient;
use actix_web::dev::Server;
use actix_web::{web, App, HttpServer};
use secrecy::Secret;
use std::net::TcpListener;
// use actix_web::middleware::Logger;
use tracing_actix_web::TracingLogger;

use crate::db::PgPool;

use crate::routes::{
    health_check::health_check, newsletter::publish_newsletter, subscriptions::subscribe,
    subscriptions_confirm::confirm,
};

pub struct Application {
    port: u16,
    server: Server,
}
impl Application {
    pub async fn build(
        port: u16,
        pool: PgPool,
        mock_server_uri: Option<String>,
    ) -> Result<Self, std::io::Error> {
        use dotenv::dotenv;
        use std::env;
        dotenv().ok();
        let application_base_url =
            env::var("APPLICATION_BASE_URL").expect("Failed to get application base url");
        let timeout = EmailClient::timeout();
        let sender =
            SubscriberEmail::parse(env::var("SENDER_EMAIL").expect("Failed to get sender email "))
                .expect("Invalid sender email address");
        let email_client = if let Some(uri) = mock_server_uri {
            EmailClient::new(
                uri,
                sender,
                Secret::new(env::var("AUTHORIZATION_TOKEN").expect("Failed to get auth token")),
                timeout,
            )
        } else {
            EmailClient::new(
                env::var("BASE_URL").expect("Failed to get base URL"),
                sender,
                Secret::new(env::var("AUTHORIZATION_TOKEN").expect("Failed to get auth token")),
                timeout,
            )
        };

        let (listener, actual_port) = if port == 0 {
            let listener = TcpListener::bind("127.0.0.1:0")?;
            let actual_port = listener.local_addr()?.port();
            (listener, actual_port)
        } else {
            let address = format!("127.0.0.1:{}", port);
            let listener = TcpListener::bind(&address)?;
            (listener, port)
        };

        let server = run(listener, pool, email_client, application_base_url)?;

        Ok(Self {
            port: actual_port,
            server,
        })
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    pub async fn run_until_stopped(self) -> Result<(), std::io::Error> {
        self.server.await
    }
}

pub struct ApplicationBaseUrl(pub String);

// We need to mark `run` as public,  It is no longer a binary entrypoint, therefore we can mark it as async, without having to use any proc-macro incantation.
pub fn run(
    listener: TcpListener,
    db_pool: PgPool,
    email_client: EmailClient,
    application_base_url: String,
) -> Result<Server, std::io::Error> {
    let application_base_url = web::Data::new(ApplicationBaseUrl(application_base_url));
    let db_pool = web::Data::new(db_pool);
    let email_client = web::Data::new(email_client);

    let server = HttpServer::new(move || {
        App::new()
            .wrap(TracingLogger::default())
            .app_data(db_pool.clone())
            .app_data(email_client.clone())
            .app_data(application_base_url.clone())
            .route("/health_check", web::get().to(health_check))
            .route("/subscriptions", web::post().to(subscribe))
            .route("/subscriptions/confirm", web::get().to(confirm))
            .route("/newsletters", web::post().to(publish_newsletter))
    })
    .listen(listener)?
    .run();
    // No .await here
    Ok(server)
}
