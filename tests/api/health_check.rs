use crate::helpers::spawn_app;
use newsletter::db::drop_database;
use reqwest::Client;

#[tokio::test]
async fn health_check_works() {
    let app = spawn_app().await;
    let client = Client::new();
    let response = client
        .get(&format!("{}/health_check", &app.address))
        .send()
        .await
        .expect("Fai led to execute request.");

    assert!(response.status().is_success());
    assert_eq!(Some(0), response.content_length());
    drop_database(&app.database_name);
}
