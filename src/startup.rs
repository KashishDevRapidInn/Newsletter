use actix_web::{web, App, HttpServer};
use std::net::TcpListener;
use actix_web::dev::Server;
// use actix_web::middleware::Logger;
use tracing_actix_web::TracingLogger;

use crate::db::PgPool;
use crate::routes::{
    health_check::health_check,
    subscriptions::subscribe,
};

// We need to mark `run` as public,  It is no longer a binary entrypoint, therefore we can mark it as async, without having to use any proc-macro incantation.
pub fn run(listener: TcpListener, db_pool: PgPool) -> Result<Server, std::io::Error> {

   let db_pool = web::Data::new(db_pool);

    let server = HttpServer::new(move|| {
    App::new()
        .wrap(TracingLogger::default())
        .app_data(db_pool.clone())
        .route("/health_check", web::get().to(health_check))
        .route("/subscriptions", web::post().to(subscribe))
    })
    .listen(listener)?
    .run();
    // No .await here
    Ok(server)
}
