use crate::domain::SubscriberEmail;
use crate::email_client::EmailClient;
use crate::middleware::reject_anonymous_users;
use actix_session::storage::RedisSessionStore;
use actix_session::SessionMiddleware;
use actix_web::cookie::Key;
use actix_web::dev::Server;
use actix_web::{web, App, HttpServer};
use actix_web_flash_messages::storage::CookieMessageStore;
use actix_web_flash_messages::FlashMessagesFramework;
use actix_web_lab::middleware::from_fn;
use secrecy::ExposeSecret;
use secrecy::Secret;
use std::net::TcpListener;

// use actix_web::middleware::Logger;
use tracing_actix_web::TracingLogger;

use crate::db::PgPool;

use crate::routes::{
    admin::{
        dashboard::admin_dashboard,
        logout::log_out,
        password::{get::change_password_form, post::change_password},
    },
    health_check::health_check,
    home::home::home,
    login::{get::login_form, post::login},
    newsletter::publish_newsletter,
    subscriptions::subscribe,
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
    ) -> Result<Self, anyhow::Error> {
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

        let redis_uri =
            Secret::new(env::var("REDIS_URI").expect("Failed to get redis configurations"));
        let server = run(
            listener,
            pool,
            email_client,
            application_base_url,
            redis_uri,
        )
        .await?;

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
async fn run(
    listener: TcpListener,
    db_pool: PgPool,
    email_client: EmailClient,
    application_base_url: String,
    redis_uri: Secret<String>,
) -> Result<Server, anyhow::Error> {
    use dotenv::dotenv;
    dotenv().ok();
    let hmac_secret = Secret::new(std::env::var("HMAC_SECRET").expect("HMAC_SECRET must be set"));
    let secret_key = Key::from(hmac_secret.expose_secret().as_bytes());
    let application_base_url = web::Data::new(ApplicationBaseUrl(application_base_url));
    let db_pool = web::Data::new(db_pool);
    let email_client = web::Data::new(email_client);
    let message_store =
        CookieMessageStore::builder(Key::from(hmac_secret.expose_secret().as_bytes())).build();
    let message_framework = FlashMessagesFramework::builder(message_store).build();
    // let redis_store = RedisSessionStore::new(redis_uri.expose_secret()).await?;
    let redis_store = RedisSessionStore::new(redis_uri.expose_secret())
        .await
        .map_err(|e| {
            eprintln!("Failed to create Redis session store: {:?}", e);
            std::io::Error::new(std::io::ErrorKind::Other, "Redis connection failed")
        })?;
    let server = HttpServer::new(move || {
        App::new()
            .wrap(message_framework.clone())
            .wrap(SessionMiddleware::new(
                redis_store.clone(),
                secret_key.clone(),
            ))
            .wrap(TracingLogger::default())
            .app_data(db_pool.clone())
            .app_data(email_client.clone())
            .app_data(application_base_url.clone())
            .route("/health_check", web::get().to(health_check))
            .route("/subscriptions", web::post().to(subscribe))
            .route("/subscriptions/confirm", web::get().to(confirm))
            .route("/newsletters", web::post().to(publish_newsletter))
            .route("/", web::get().to(home))
            .route("/login", web::get().to(login_form))
            .route("/login", web::post().to(login))
            .service(
                web::scope("/admin")
                    .wrap(from_fn(reject_anonymous_users))
                    .route("/dashboard", web::get().to(admin_dashboard))
                    .route("/password", web::get().to(change_password_form))
                    .route("/password", web::post().to(change_password))
                    .route("/logout", web::post().to(log_out)),
            )
    })
    .listen(listener)?
    .run();
    // No .await here
    Ok(server)
}
