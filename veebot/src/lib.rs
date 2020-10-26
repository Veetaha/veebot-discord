pub(crate) mod audio_queue;
pub(crate) mod commands;
pub(crate) mod di;
pub(crate) mod error;
pub(crate) mod util;
pub(crate) mod yt;

pub(crate) use crate::error::{err, Result};
use futures::FutureExt;
use serde::Deserialize;
use serenity::{
    async_trait,
    client::{Client, Context, EventHandler},
    framework::StandardFramework,
    http::Http,
    model::gateway::Ready,
};
use std::{iter, sync::Arc};
use tracing::{info, warn};

#[derive(Debug)]
struct Handler;

#[async_trait]
impl EventHandler for Handler {
    async fn ready(&self, _: Context, ready_event: Ready) {
        info!(?ready_event, "🚀 Discord bot is listening!");
    }
}

#[derive(Deserialize)]
pub struct Config {
    discord_bot_token: String,
    cmd_prefix: String,
    yt_data_api_key: String,
}

/// Run the discord bot event loop
pub async fn run(config: Config) -> eyre::Result<()> {
    let http = Http::new_with_token(&config.discord_bot_token);
    let bot_app_info = http.get_current_application_info().await?;

    let framework = StandardFramework::new()
        .configure(|c| {
            c.owners(iter::once(bot_app_info.owner.id).collect())
                .prefix(&config.cmd_prefix)
        })
        .group(&commands::META_GROUP)
        .group(&commands::audio::AUDIO_GROUP);

    let mut client = Client::new(config.discord_bot_token)
        .framework(framework)
        .event_handler(Handler)
        // FIXME: configure proper intents
        // .add_intent(GatewayIntents::)
        .await?;

    // Inject the necessary dependencies
    {
        let mut data = client.data.write().await;
        data.insert::<di::ClientVoiceManagerToken>(Arc::clone(&client.voice_manager));
        data.insert::<di::YtServiceToken>(Arc::new(yt::YtService::new(config.yt_data_api_key)));
    }

    futures::select! {
        it = client.start().fuse() => it?,
        it = abort_signal().fuse() => {
            client.shard_manager.lock().await.shutdown_all().await;
            it?
        },
    };

    Ok(())
}

async fn abort_signal() -> eyre::Result<()> {
    tokio::signal::ctrl_c().await?;
    warn!("Ctrl-c: Aborting...");
    Ok(())
}
