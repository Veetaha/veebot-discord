use crate::{
    audio_queue::{AudioQueueCmd, AudioTrackOrder},
    di::{self, DiExt},
};
use futures::channel::mpsc;
use serenity::{
    client::Context, framework::standard::macros::group, framework::standard::Args,
    model::channel::Message,
};
use veebot_cmd::veebot_cmd;

#[group]
#[commands(play, skip, now_playing, queue, pause, resume, clear)]
pub(crate) struct Audio;

#[veebot_cmd]
#[aliases("p")]
async fn play(ctx: &Context, msg: &Message, mut args: Args) -> crate::Result<()> {
    let task_send = get_or_create_audio_track_queue(ctx, msg).await?;

    let yt_vid = {
        let yt = ctx.data.expect_dep::<di::YtServiceToken>().await;
        match args.single::<url::Url>() {
            Ok(it) => yt.find_video_by_url(&it).await?,
            Err(_) => yt.find_video_by_query(args.remains().unwrap_or("")).await?,
        }
    };

    let order = AudioTrackOrder {
        meta: yt_vid,
        ordered_by: msg.clone(),
    };

    task_send
        .unbounded_send(AudioQueueCmd::PlayTrack(order))
        .unwrap();
    Ok(())
}

#[veebot_cmd]
#[aliases("s", "fs")]
async fn skip(ctx: &Context, msg: &Message, mut args: Args) -> crate::Result<()> {
    let task_send = get_or_create_audio_track_queue(ctx, msg).await?;

    let index: usize = if args.is_empty() {
        0
    } else {
        args.single().map_err(|err| crate::err!(ParseInt(err)))?
    };

    task_send
        .unbounded_send(AudioQueueCmd::SkipTrack {
            source: msg.clone(),
            index,
        })
        .unwrap();

    Ok(())
}

#[veebot_cmd]
#[aliases("np")]
async fn now_playing(ctx: &Context, msg: &Message) -> crate::Result<()> {
    let task_send = get_or_create_audio_track_queue(ctx, msg).await?;

    task_send
        .unbounded_send(AudioQueueCmd::ShowNowPlaying {
            source: msg.clone(),
        })
        .unwrap();

    Ok(())
}

#[veebot_cmd]
#[aliases("q")]
async fn queue(ctx: &Context, msg: &Message) -> crate::Result<()> {
    let task_send = get_or_create_audio_track_queue(ctx, msg).await?;

    task_send
        .unbounded_send(AudioQueueCmd::ShowQueue {
            source: msg.clone(),
        })
        .unwrap();

    Ok(())
}

#[veebot_cmd]
async fn pause(ctx: &Context, msg: &Message) -> crate::Result<()> {
    let task_send = get_or_create_audio_track_queue(ctx, msg).await?;

    task_send
        .unbounded_send(AudioQueueCmd::Pause {
            source: msg.clone(),
        })
        .unwrap();

    Ok(())
}

#[veebot_cmd]
async fn resume(ctx: &Context, msg: &Message) -> crate::Result<()> {
    let task_send = get_or_create_audio_track_queue(ctx, msg).await?;

    task_send
        .unbounded_send(AudioQueueCmd::Resume {
            source: msg.clone(),
        })
        .unwrap();

    Ok(())
}

#[veebot_cmd]
#[aliases("c")]
async fn clear(ctx: &Context, msg: &Message) -> crate::Result<()> {
    let task_send = get_or_create_audio_track_queue(ctx, msg).await?;

    task_send
        .unbounded_send(AudioQueueCmd::Clear {
            source: msg.clone(),
        })
        .unwrap();

    Ok(())
}

async fn get_or_create_audio_track_queue(
    ctx: &Context,
    msg: &Message,
) -> crate::Result<mpsc::UnboundedSender<AudioQueueCmd>> {
    let guild_id = msg.guild_id.ok_or_else(|| crate::err!(UserNotInGuild))?;
    Ok(ctx
        .data
        .expect_dep::<di::AudioServiceToken>()
        .await
        .get_or_create_queue(guild_id)
        .await)
}
