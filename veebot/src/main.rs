use eyre::{Result, WrapErr as _};
use std::env;

#[tokio::main]
async fn main() -> Result<()> {
    simple_eyre::install().unwrap();

    if let Err(_) = dotenv::dotenv() {
        eprintln!("Dotenv config was not found, ignoring this...")
    }

    tracing::subscriber::set_global_default(
        tracing_subscriber::FmtSubscriber::builder()
            .with_env_filter(tracing_subscriber::EnvFilter::from_env("VEEBOT_LOG"))
            .with_target(true)
            .with_ansi(env::var("COLORS").as_deref() != Ok("0"))
            .finish(),
    )
    .unwrap();

    let config: veebot::Config = envy::from_env().wrap_err("invalid config")?;

    veebot::run(config).await?;

    Ok(())
}
