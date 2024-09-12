use crate::helpers::assert_is_redirect_to;
use crate::helpers::spawn_app;
use reqwest::header::HeaderValue;
use std::collections::HashSet;

#[tokio::test]
async fn an_error_flash_message_is_set_on_failure() {
    // Part-1(Post /login)Try to login
    let app = spawn_app().await;

    let login_body = serde_json::json!({
    "username": "random-username",
    "password": "random-password"
    });
    let response = app.post_login(&login_body).await;

    assert_eq!(response.status().as_u16(), 303);
    assert_is_redirect_to(&response, "/login");
    // let cookies: HashSet<_> = response
    //     .headers()
    //     .get_all("Set-Cookie")
    //     .into_iter()
    //     .collect();
    // assert!(cookies.contains(&HeaderValue::from_str("_flash=Authentication failed").unwrap()));
    // let flash_cookie = response.cookies().find(|c| c.name() == "_flash").unwrap();
    // assert_eq!(flash_cookie.value(), "Authentication failed");

    // Part-2(Get /login)  Follow the redirect

    let html_page = app.get_login_html().await;
    assert!(html_page.contains(r#"<p><i>Authentication failed</i></p>"#));

    // Part-2 - Reload the login page
    let html_page = app.get_login_html().await;
    assert!(!html_page.contains(r#"<p><i>Authentication failed</i></p>"#));
}
