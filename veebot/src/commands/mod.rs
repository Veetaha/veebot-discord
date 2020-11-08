//! Discord commands root module

pub(crate) mod audio;

use std::collections::HashSet;

use crate::{
    derpibooru,
    di::{self, DiExt},
};
use itertools::Itertools;
use serenity::{
    client::Context,
    framework::standard::{macros::group, Args},
    model::channel::Message,
};
use veebot_cmd::veebot_cmd;

#[group]
#[commands(pink)]
pub(crate) struct Meta;

#[veebot_cmd]
async fn pink(ctx: &Context, msg: &Message) -> crate::Result<()> {
    msg.channel_id.say(&ctx, "(\\ Ponk").await?;

    Ok(())
}

#[group]
#[commands(pony)]
pub(crate) struct General;

#[veebot_cmd]
async fn pony(ctx: &Context, msg: &Message, args: Args) -> crate::Result<()> {
    // TODO: escape returned video/image tags for markdown
    let tags: HashSet<derpibooru::Tag> = args
        .raw_quoted()
        .map(|it| it.parse())
        .collect::<crate::Result<_>>()?;

    let derpibooru = ctx.data.expect_dep::<di::DerpibooruServiceToken>().await;

    let footer = "Powered by derpibooru.org";
    let media = match derpibooru.fetch_random_pony_media(&tags).await? {
        Some(it) => it,
        None => {
            msg.channel_id
                .send_message(ctx, |it| {
                    it.embed(|it| {
                        it.title("Pony was not found.")
                            .description(format_args!(
                                "Failed to fetch pony with tags `[{}]`.",
                                tags.iter().format(", ")
                            ))
                            .footer(|it| it.text(footer))
                    })
                })
                .await?;
            return Ok(());
        }
    };

    let media_url = &media.representations.full;

    msg.channel_id
        .send_message(ctx, |it| {
            it.embed(|it| {
                it.title(format_args!("Random pony for **{}**", msg.author.name))
                    .description(format_args!(
                        "**Score:** *{}*\n**Created:**: *{}*\n**Tags:** *```{}```*\n",
                        media.score,
                        timeago::Formatter::new().convert_chrono(
                            chrono::DateTime::<chrono::Utc>::from_utc(
                                media.created_at,
                                chrono::Utc
                            ),
                            chrono::Utc::now(),
                        ),
                        media.tags.join(", "),
                    ))
                    .url(media.webpage_url())
                    .footer(|it| it.text(footer));

                if media.mime_type.is_image() {
                    it.image(media_url);
                }
                it
            })
        })
        .await?;

    if !media.mime_type.is_image() {
        msg.channel_id
            .send_message(ctx, |it| it.content(media_url))
            .await?;
    }

    Ok(())
}
