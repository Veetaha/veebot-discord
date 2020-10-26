use eyre::{Result, WrapErr as _};

#[tokio::main]
async fn main() -> Result<()> {
    color_eyre::install().unwrap();
    dotenv::dotenv()?;

    tracing::subscriber::set_global_default(
        tracing_subscriber::FmtSubscriber::builder()
            .with_env_filter(tracing_subscriber::EnvFilter::from_env("VEEBOT_LOG"))
            .with_target(true)
            .finish(),
    )
    .unwrap();

    let config: veebot::Config = envy::from_env().wrap_err("invalid config")?;

    veebot::run(config).await?;

    Ok(())
}
