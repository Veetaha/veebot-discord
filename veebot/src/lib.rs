pub(crate) mod audio_queue;
pub(crate) mod commands;
pub(crate) mod derpibooru;
pub(crate) mod di;
pub(crate) mod error;
pub(crate) mod gelbooru;
pub(crate) mod util;
pub(crate) mod yt;

pub(crate) use crate::error::{err, Error, ErrorKind, Result};
use audio_queue::AudioService;
use futures::FutureExt;
use serde::Deserialize;
use serenity::{
    async_trait,
    client::{Client, Context, EventHandler},
    framework::standard::help_commands,
    framework::standard::{macros::help, Args, CommandGroup, CommandResult, HelpOptions},
    framework::StandardFramework,
    http::Http,
    model::channel::Message,
    model::gateway::Ready,
    model::id::UserId,
};
use std::{collections::HashSet, iter, sync::Arc};
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
    derpibooru_api_key: String,
    derpibooru_always_on_tags: HashSet<String>,
    derpibooru_filter: String,
    gelbooru_api_key: String,
    gelbooru_user_id: String,
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
        .group(&commands::GENERAL_GROUP)
        .group(&commands::META_GROUP)
        .group(&commands::audio::AUDIO_GROUP)
        .help(&HELP);

    let mut client = Client::new(config.discord_bot_token)
        .framework(framework)
        .event_handler(Handler)
        // FIXME: configure proper intents
        // .add_intent(GatewayIntents::)
        .await?;

    let derpibooru_service = Arc::new(derpibooru::DerpibooruService::new(
        config.derpibooru_api_key,
        config.derpibooru_filter,
        config
            .derpibooru_always_on_tags
            .into_iter()
            .map(|it| it.parse().unwrap())
            .collect(),
    ));

    let audio_service = Arc::new(AudioService::new(
        Arc::clone(&client.voice_manager),
        Arc::clone(&client.cache_and_http),
        Arc::clone(&derpibooru_service),
    ));

    // Inject the necessary dependencies
    {
        let mut data = client.data.write().await;
        data.insert::<di::ClientVoiceManagerToken>(Arc::clone(&client.voice_manager));
        data.insert::<di::YtServiceToken>(Arc::new(yt::YtService::new(config.yt_data_api_key)));
        data.insert::<di::DerpibooruServiceToken>(derpibooru_service);
        data.insert::<di::AudioServiceToken>(audio_service);
        data.insert::<di::GelbooruServiceToken>(Arc::new(gelbooru::GelbooruService::new(
            config.gelbooru_api_key,
            config.gelbooru_user_id,
        )))
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

// The framework provides two built-in help commands for you to use.
// But you can also make your own customized help command that forwards
// to the behaviour of either of them.
#[help]
async fn help(
    context: &Context,
    msg: &Message,
    args: Args,
    help_options: &'static HelpOptions,
    groups: &[&'static CommandGroup],
    owners: HashSet<UserId>,
) -> CommandResult {
    let _ = help_commands::with_embeds(context, msg, args, help_options, groups, owners).await;
    Ok(())
}
