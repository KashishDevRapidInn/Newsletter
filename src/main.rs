use newsletter::telemetry::{get_subscriber, init_subscriber};
use newsletter::startup::build;

#[tokio::main]
async fn main() -> std::io::Result<()> {

    let subscriber = get_subscriber("newsletter_kk".into(), "info".into(), std::io::stdout);
    init_subscriber(subscriber);

    let server= build().await?;
    server.await?;
    Ok(())

}