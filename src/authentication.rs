use crate::{
    db::PgPool,
    schema::users::{self, dsl::*},
    telemetry::spawn_blocking_with_tracing,
};
use anyhow::Context;
use argon2::password_hash::SaltString;
use argon2::{Algorithm, Argon2, Params, PasswordHash, PasswordHasher, PasswordVerifier, Version};
use diesel::prelude::*;
use secrecy::{ExposeSecret, Secret};
use uuid::Uuid;

#[derive(thiserror::Error, Debug)]
pub enum AuthError {
    #[error("Invalid credentials.")]
    InvalidCredentials(#[source] anyhow::Error),
    #[error(transparent)]
    UnexpectedError(#[from] anyhow::Error),
}
pub struct Credentials {
    pub username: String,
    pub password: Secret<String>,
}
#[tracing::instrument(name = "Get stored credentials", skip(user_name, pool))]
async fn get_stored_credentials(
    user_name: &str,
    pool: &PgPool,
) -> Result<(uuid::Uuid, Secret<String>), anyhow::Error> {
    let mut conn = pool.get().expect("Failed to get db connection from pool");

    let row = users::table
        .filter(users::username.eq(user_name))
        .select((users::user_id, users::password_hash))
        .load::<(Uuid, String)>(&mut conn)
        .optional()
        .context("Failed to perform a query to retrieve stored credentials.")?;

    let (id_user, expected_hash_password) = match row.and_then(|r| r.into_iter().next()) {
        //since .load returns a vector and_then is used to handle the Option returned by optional(), and into_iter().next() gets the first item from the vector.
        Some(row) => (row.0, row.1),
        None => {
            return Err(anyhow::anyhow!("Invalid username or password."));
        }
    };
    Ok((id_user, Secret::new(expected_hash_password)))
}

#[tracing::instrument(
    name = "Verify password hash",
    skip(expected_password_hash, password_candidate)
)]
fn verify_password_hash(
    expected_password_hash: Secret<String>,
    password_candidate: Secret<String>,
) -> Result<(), AuthError> {
    let expected_password_hash = PasswordHash::new(expected_password_hash.expose_secret())
        .context("Failed to parse hash in PHC string format.")?;
    // println!("{}", expected_password_hash);
    // println!("{}", password_candidate.expose_secret());

    Argon2::default()
        .verify_password(
            password_candidate.expose_secret().as_bytes(),
            &expected_password_hash,
        )
        .context("Invalid password.")
        .map_err(AuthError::InvalidCredentials)
}

// validating user credentials using Argon2 hashing and PHC string format
#[tracing::instrument(name = "Validate credentials", skip(credentials, pool))]
pub async fn validate_credentials(
    credentials: Credentials,
    pool: &PgPool,
) -> Result<uuid::Uuid, AuthError> {
    let mut id_user = None;
    let mut expected_password_hash = Secret::new(
        "$argon2id$v=19$m=15000,t=2,p=1$\
        gZiV/M1gPc22ElAH/Jh1Hw$\
        CWOrkoo7oJBQ/iyh7uJ0LO2aLEfrHwTWllSAxT0zRno"
            .to_string(),
    );

    if let Ok((stored_user_id, stored_expected_password_hash)) =
        get_stored_credentials(&credentials.username, &pool)
            .await
            .map_err(AuthError::InvalidCredentials)
    {
        id_user = Some(stored_user_id);
        expected_password_hash = stored_expected_password_hash;
    }

    let _result = spawn_blocking_with_tracing(move || {
        verify_password_hash(expected_password_hash, credentials.password)
    })
    .await
    .context("Failed to spawn blocking task.")??;

    // match result {
    //     Ok(result) => Ok(user_id),
    //     Err(e) => Err(PublishError::AuthError(e.into())),
    // }

    // println!("{:?}", result.unwrap());
    // Ok(user_id)
    id_user
        .ok_or_else(|| anyhow::anyhow!("Unknown username."))
        .map_err(AuthError::InvalidCredentials)
}

#[tracing::instrument(name = "Change password", skip(password, pool, id_user))]
pub async fn change_password(
    id_user: uuid::Uuid,
    password: Secret<String>,
    pool: &PgPool,
) -> Result<(), anyhow::Error> {
    let hashed_password = spawn_blocking_with_tracing(move || compute_password_hash(password))
        .await?
        .context("Failed to hash password")?;

    let mut conn = pool.get().expect("Couldn't get db connection from Pool");
    diesel::update(users::table.filter(users::user_id.eq(id_user)))
        .set(users::password_hash.eq(hashed_password.expose_secret()))
        .execute(&mut conn)
        .map_err(|e| anyhow::anyhow!("Failed to update password in database: {}", e))?;

    Ok(())
}

fn compute_password_hash(password: Secret<String>) -> Result<Secret<String>, anyhow::Error> {
    let salt_argon = SaltString::generate(&mut rand::thread_rng());
    let hashed_password = Argon2::default()
        .hash_password(password.expose_secret().as_bytes(), &salt_argon)
        .unwrap()
        .to_string();
    Ok(Secret::new(hashed_password))
}
