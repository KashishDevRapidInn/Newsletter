use newsletter::telemetry::{get_subscriber, init_subscriber};
use newsletter::startup::Application;
use newsletter::db::establish_connection;

#[tokio::main]
async fn main() -> std::io::Result<()> {

    let subscriber = get_subscriber("newsletter_kk".into(), "info".into(), std::io::stdout);
    init_subscriber(subscriber);

    let database_name = "newsletter";
    let pool= establish_connection(database_name);
    let application = Application::build(8080, pool.clone(), None).await?;
    application.run_until_stopped().await?;
    Ok(())

}