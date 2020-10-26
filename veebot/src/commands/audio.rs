use crate::{
    audio_queue::AudioTrack,
    di::{self, DiExt},
};
use core::fmt;
use hhmmss::Hhmmss;
use serenity::{
    client::Context, framework::standard::macros::group, framework::standard::Args,
    model::channel::Message, model::user::User, voice,
};
use veebot_cmd::veebot_cmd;

#[group]
#[commands(play)]
pub(crate) struct Audio;

fn _join_user_voice_channel(_user: User) {}

#[veebot_cmd]
async fn play(ctx: &Context, msg: &Message, mut args: Args) -> crate::Result<()> {
    let yt_vid = {
        let yt = ctx.data.expect_dep::<di::YtServiceToken>().await;
        match args.single::<url::Url>() {
            Ok(it) => yt.find_video_by_url(&it).await?,
            Err(_) => yt.find_video_by_query(args.remains().unwrap_or("")).await?,
        }
    };

    let guild = msg
        .guild(&ctx)
        .await
        .ok_or_else(|| crate::err!(UserNotInGuild))?;

    let channel_id = guild
        .voice_states
        .get(&msg.author.id)
        .and_then(|it| it.channel_id)
        .ok_or_else(|| crate::err!(UserNotInVoiceChanel))?;

    let mut voice_mgr = ctx.data.lock_dep::<di::ClientVoiceManagerToken>().await;

    let handler = match voice_mgr.join(guild.id, channel_id) {
        Some(it) => it,
        None => return Err(crate::err!(JoinVoiceChannel(channel_id.name(ctx).await)))?,
    };

    let source = voice::ytdl(yt_vid.url().as_str())
        .await
        .map_err(|err| crate::err!(AudioStart(err)))?;

    let source = handler.play_only(source);

    let track = AudioTrack {
        meta: yt_vid,
        source,
        ordered_by: msg.author.clone(),
    };

    let footer_text = active_track_footer_text(&track).await;

    msg.channel_id
        .send_message(ctx, |it| {
            it.embed(|it| {
                it.description(format_args!(
                    "Now playing:\n[**{}**]({}) - [**\"{}\"**]({})",
                    track.meta.channel_title(),
                    track.meta.channel_url(),
                    track.meta.title(),
                    track.meta.url(),
                ))
                .thumbnail(&track.meta.thumbnail_url())
                .footer(|it| it.text(footer_text).icon_url(track.ordered_by.face()))
            })
        })
        .await?;

    Ok(())
}

/// Creates discord embed footer text part for the currently playing track.
async fn active_track_footer_text(track: &AudioTrack) -> String {
    let played_duration = format_duration(&track.source.lock().await.position);
    let total_track_duration = format_duration(&track.meta.duration());
    format!(
        "ordered by {} ({} / {})",
        // FIXME: use `.nick_in(guild_id)`
        track.ordered_by.name,
        played_duration,
        total_track_duration,
    )
}

/// Returns duration in a colon separated string format.
fn format_duration(duration: &impl Hhmmss) -> impl fmt::Display {
    // Unfortunately chrono doesn't have anything useful for formatting durations
    // FIXME: use chrono means of fomratting durations once this is added to the lib:
    // https://github.com/chronotope/chrono/issues/197#issuecomment-716257398
    let rendered = duration.hhmmss();

    // Remove unnecessary leading zeros for hours (most of the tracks are within the minutes timespan)
    match rendered.strip_prefix("00:") {
        Some(it) => it.to_owned(),
        None => rendered,
    }
}
