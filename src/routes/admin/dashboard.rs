use crate::db::PgPool;
use crate::db_models::User;
use crate::schema::users::dsl::*;
use crate::session_state::TypedSession;
use crate::utils::e500;
use actix_web::http::header::{ContentType, LOCATION};
use actix_web::web;
use actix_web::{Error as ActixError, HttpResponse};
use anyhow::Context;
use diesel::prelude::*;
use diesel::r2d2::{self, ConnectionManager};
use diesel::PgConnection;
use uuid::Uuid;

pub async fn admin_dashboard(
    session: TypedSession,
    pool: web::Data<PgPool>,
) -> Result<HttpResponse, actix_web::Error> {
    let user_name = if let Some(id_user) = session.get_user_id().map_err(e500)? {
        get_username(id_user, &pool).await.map_err(e500)?
    } else {
        return Ok(HttpResponse::SeeOther()
            .insert_header((LOCATION, "/login"))
            .finish());
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
                <p>Available actions:</p>
                <ol>
                    <li><a href="/admin/password">Change password</a></li>
                </ol>
                <ol>
                    <li><a href="/admin/password">Change password</a></li>
                    <li>
                    <form name="logoutForm" action="/admin/logout" method="post">
                    <input type="submit" value="Logout">
                    </form>
                    </li>
                </ol>
            </body>
            </html>"#
        )))
}
#[tracing::instrument(name = "Get username", skip(pool))]
pub async fn get_username(id_user: Uuid, pool: &PgPool) -> Result<String, anyhow::Error> {
    let mut conn = pool.get().expect("Failed to get db connection from pool");
    let user = users.filter(user_id.eq(id_user)).first::<User>(&mut conn)?;
    Ok(user.username)
}
