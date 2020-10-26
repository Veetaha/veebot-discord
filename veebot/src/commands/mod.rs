//! Discord commands root module

pub(crate) mod audio;

use serenity::{
    client::Context,
    framework::standard::macros::{command, group},
    framework::standard::CommandResult,
    model::channel::Message,
};

#[group]
#[commands(pink)]
pub(crate) struct Meta;

#[command]
async fn pink(ctx: &Context, msg: &Message) -> CommandResult {
    msg.channel_id.say(&ctx, "Ponk /)").await?;

    Ok(())
}
