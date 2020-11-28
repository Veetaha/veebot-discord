//! Discord commands root module

pub(crate) mod audio;
pub(crate) mod image;

use std::collections::HashSet;

use crate::{
    di::{self, DiExt},
    util::ThemeTag,
};
use itertools::Itertools;
use serenity::{
    client::Context,
    framework::standard::{macros::group, Args},
    model::channel::Message,
    utils::MessageBuilder,
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
#[commands(pony, anime)]
pub(crate) struct General;

#[veebot_cmd]
async fn pony(ctx: &Context, msg: &Message, args: Args) -> crate::Result<()> {
    let tags: HashSet<ThemeTag> = args
        .raw_quoted()
        .map(|it| it.parse())
        .collect::<crate::Result<_>>()?;

    let derpibooru = ctx.data.expect_dep::<di::DerpibooruServiceToken>().await;

    let footer = "Powered by derpibooru.org";
    let media = match derpibooru.fetch_random_media(tags.iter().cloned()).await? {
        Some(it) => it,
        None => {
            msg.channel_id
                .send_message(ctx, |it| {
                    it.embed(|it| {
                        it.title("Pony was not found.")
                            .description(
                                MessageBuilder::new()
                                    .push("Failed to fetch pony with tags ")
                                    .push_mono_safe(format_args!("[{}]", tags.iter().format(", "))),
                            )
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
                it.title(
                    MessageBuilder::new()
                        .push("Random pony for ")
                        .push_bold_safe(&msg.author.name),
                )
                .description(
                    MessageBuilder::new()
                        .push_bold("Score:")
                        .push(" ")
                        .push_italic_line(media.score)
                        .push_bold("Created:")
                        .push(" ")
                        .push_italic_line_safe(timeago::Formatter::new().convert_chrono(
                            chrono::DateTime::<chrono::Utc>::from_utc(
                                media.created_at,
                                chrono::Utc,
                            ),
                            chrono::Utc::now(),
                        ))
                        .push_bold_line("Tags:")
                        .push_italic_line_safe(
                            MessageBuilder::new().push_codeblock_safe(media.tags.join(", "), None),
                        ),
                )
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

#[veebot_cmd]
async fn anime(ctx: &Context, msg: &Message, args: Args) -> crate::Result<()> {
    let tags: HashSet<ThemeTag> = args
        .raw_quoted()
        .map(|it| it.parse())
        .collect::<crate::Result<_>>()?;

    let gelbooru = ctx.data.expect_dep::<di::GelbooruServiceToken>().await;

    let footer = "Powered by gelbooru.com";
    let media = match gelbooru.fetch_random_media(tags.iter().cloned()).await? {
        Some(it) => it,
        None => {
            msg.channel_id
                .send_message(ctx, |it| {
                    it.embed(|it| {
                        it.title("Anime was not found.")
                            .description(
                                MessageBuilder::new()
                                    .push("Failed to fetch anime with tags ")
                                    .push_mono_safe(format_args!("[{}]", tags.iter().format(", "))),
                            )
                            .footer(|it| it.text(footer))
                    })
                })
                .await?;
            return Ok(());
        }
    };

    let media_url = &media.file_url;

    msg.channel_id
        .send_message(ctx, |it| {
            it.embed(|it| {
                it.title(
                    MessageBuilder::new()
                        .push("Random anime for ")
                        .push_bold_safe(&msg.author.name),
                )
                .description(
                    MessageBuilder::new()
                        .push_bold("Score:")
                        .push(" ")
                        .push_italic_line(media.score)
                        .push_bold("Created:")
                        .push(" ")
                        .push_italic_line_safe(&media.created_at)
                        .push_bold_line("Tags:")
                        .push_italic_line_safe(
                            MessageBuilder::new().push_codeblock_safe(&media.tags, None),
                        ),
                )
                .url(media.webpage_url())
                .footer(|it| it.text(footer));

                it.image(media_url);
                it
            })
        })
        .await?;

    Ok(())
}
