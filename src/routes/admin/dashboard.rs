use crate::db::PgPool;
use crate::db_models::User;
use crate::schema::users::dsl::*;
use actix_session::Session;
use actix_web::http::header::ContentType;
use actix_web::web;
use actix_web::{Error as ActixError, HttpResponse};
use anyhow::Context;
use diesel::prelude::*;
use diesel::r2d2::{self, ConnectionManager};
use diesel::PgConnection;
use uuid::Uuid;

fn e500<T>(e: T) -> actix_web::Error
where
    T: std::fmt::Debug + std::fmt::Display + 'static,
{
    actix_web::error::ErrorInternalServerError(e)
}
pub async fn admin_dashboard(
    session: Session,
    pool: web::Data<PgPool>,
) -> Result<HttpResponse, actix_web::Error> {
    let user_name = if let Some(id_user) = session.get::<Uuid>("user_id").map_err(e500)? {
        get_username(id_user, &pool).await.map_err(e500)?
    } else {
        todo!()
    };
    Ok(HttpResponse::Ok()
        .content_type(ContentType::html())
        .body(format!(
            r#"<!DOCTYPE html>
<html lang="en">
<head>
<meta http-equiv="content-type" content="text/html; charset=utf-8">
<title>Admin dashboard</title>
</head>
<body>
<p>Welcome {user_name}!</p>
</body>
</html>"#
        )))
}
#[tracing::instrument(name = "Get username", skip(pool))]
async fn get_username(id_user: Uuid, pool: &PgPool) -> Result<String, anyhow::Error> {
    let mut conn = pool.get().expect("Failed to get db connection from pool");
    let user = users.filter(user_id.eq(id_user)).first::<User>(&mut conn)?;
    Ok(user.username)
}
