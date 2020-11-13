//! Audio tracks queue implementation

use crate::{
    derpibooru::{rpc::Image, DerpibooruService},
    yt::YtVideo,
};
use futures::{
    channel::{mpsc, oneshot},
    FutureExt, StreamExt,
};
use hhmmss::Hhmmss;
use serenity::{
    async_trait,
    builder::{CreateEmbed, CreateMessage},
    client::{bridge::voice::ClientVoiceManager, Cache},
    http::Http,
    model::prelude::CurrentUser,
    model::{
        channel::{ChannelType, Message},
        id::{ChannelId, GuildId},
    },
    prelude::{Mutex, RwLock},
    utils::MessageBuilder,
    voice::{self, Audio, AudioSource},
    CacheAndHttp,
};
use std::{
    collections::hash_map::Entry, collections::HashMap, collections::VecDeque, fmt, sync::Arc, time,
};
use tracing::{debug, info, warn};

/// Inherently atomic
pub(crate) struct AudioService {
    derpibooru: Arc<DerpibooruService>,
    voice_mgr: Arc<Mutex<ClientVoiceManager>>,
    cache_and_http: Arc<CacheAndHttp>,
    queues: RwLock<HashMap<GuildId, mpsc::UnboundedSender<AudioQueueCmd>>>,
    bot_user: Arc<CurrentUser>,
}

impl AudioService {
    pub(crate) fn new(
        voice_mgr: Arc<Mutex<ClientVoiceManager>>,
        cache_and_http: Arc<CacheAndHttp>,
        derpibooru: Arc<DerpibooruService>,
        bot_user: Arc<CurrentUser>,
    ) -> Self {
        AudioService {
            voice_mgr,
            cache_and_http,
            queues: Default::default(),
            derpibooru,
            bot_user,
        }
    }

    pub(crate) async fn get_or_create_queue(
        &self,
        guild_id: GuildId,
    ) -> mpsc::UnboundedSender<AudioQueueCmd> {
        if let Some(it) = self.queues.read().await.get(&guild_id) {
            return it.clone();
        }
        match self.queues.write().await.entry(guild_id) {
            Entry::Occupied(it) => it.get().clone(),
            Entry::Vacant(it) => it
                .insert(AudioTrackQueue::run(
                    guild_id,
                    Arc::clone(&self.voice_mgr),
                    &self.cache_and_http,
                    Arc::clone(&self.derpibooru),
                    Arc::clone(&self.bot_user),
                ))
                .clone(),
        }
    }
}

pub(crate) struct ActiveAudioTrack {
    pub(crate) order: AudioTrackOrder,
    pub(crate) source: Arc<Mutex<Audio>>,
    finish_recv: oneshot::Receiver<()>,
}

pub(crate) struct AudioTrackOrder {
    pub(crate) meta: YtVideo,
    pub(crate) ordered_by: Message,
}

struct AudioTrackQueue {
    orders: VecDeque<AudioTrackOrder>,
    active_track: Option<ActiveAudioTrack>,
    voice_mgr: Arc<Mutex<ClientVoiceManager>>,
    guild_id: GuildId,
    cache: Arc<Cache>,
    http: Arc<Http>,
    derpibooru: Arc<DerpibooruService>,
    bot_user: Arc<CurrentUser>,
}

pub(crate) enum AudioQueueCmd {
    PlayTrack(AudioTrackOrder),
    SkipTrack { index: usize, source: Message },
    ShowNowPlaying { source: Message },
    ShowQueue { source: Message },
    Pause { source: Message },
    Resume { source: Message },
    Clear { source: Message },
}

impl AudioQueueCmd {
    fn source_msg(&self) -> &Message {
        match self {
            AudioQueueCmd::PlayTrack(it) => &it.ordered_by,
            AudioQueueCmd::SkipTrack { source, .. } => source,
            AudioQueueCmd::ShowNowPlaying { source, .. } => source,
            AudioQueueCmd::ShowQueue { source, .. } => source,
            AudioQueueCmd::Pause { source, .. } => source,
            AudioQueueCmd::Resume { source, .. } => source,
            AudioQueueCmd::Clear { source, .. } => source,
        }
    }
}

impl AudioTrackQueue {
    async fn send_message<'a, F>(&self, channel: ChannelId, f: F) -> crate::Result<()>
    where
        for<'b> F: FnOnce(&'b mut CreateMessage<'a>) -> &'b mut CreateMessage<'a>,
    {
        channel.send_message(&self.http, f).await?;
        Ok(())
    }

    async fn send_embed(
        &self,
        channel: ChannelId,
        f: impl FnOnce(&mut CreateEmbed) -> &mut CreateEmbed,
    ) -> crate::Result<()> {
        self.send_message(channel, |it| it.embed(f)).await?;
        Ok(())
    }

    async fn out_channel(&self) -> ChannelId {
        let guild = self.guild_id.to_guild_cached(&self.cache).await.unwrap();
        match guild.system_channel_id {
            Some(it) => it,
            None => *guild
                .channels
                .iter()
                .find(|(&id, it)| {
                    it.kind == ChannelType::Text
                        && guild
                            .user_permissions_in(id, self.bot_user.id)
                            .send_messages()
                })
                .map(|(id, _)| id)
                .unwrap_or_else(|| todo!()),
        }
    }

    pub(crate) fn run(
        guild_id: GuildId,
        voice_mgr: Arc<Mutex<ClientVoiceManager>>,
        cah: &CacheAndHttp,
        derpibooru: Arc<DerpibooruService>,
        bot_user: Arc<CurrentUser>,
    ) -> mpsc::UnboundedSender<AudioQueueCmd> {
        let (cmd_send, cmd_recv) = mpsc::unbounded();
        let cache = Arc::clone(&cah.cache);
        let http = Arc::clone(&cah.http);
        tokio::spawn(async move {
            Self {
                derpibooru,
                orders: VecDeque::new(),
                active_track: None,
                bot_user,
                voice_mgr,
                guild_id,
                cache,
                http,
            }
            .run_event_loop(cmd_recv)
            .await;
        });
        cmd_send
    }

    async fn run_event_loop(&mut self, mut cmd_recv: mpsc::UnboundedReceiver<AudioQueueCmd>) {
        loop {
            let cmd = match &mut self.active_track {
                None => cmd_recv.next().await,
                Some(ActiveAudioTrack { finish_recv, .. }) => futures::select! {
                    it = cmd_recv.next() => it,
                    it = finish_recv.fuse() => {
                        if let Ok(()) = it {
                            let _ = self.show_track_finished(&self.active_track.as_ref().unwrap().order).await;
                            self.play_next_track().await;
                        } else {
                            debug!("Track canceled");
                        }
                        continue;
                    },
                },
            };

            let cmd = match cmd {
                Some(it) => it,
                None => {
                    info!(
                        "Audio queue task sender returned `None`, shutting down the event loop..."
                    );
                    return;
                }
            };

            let source_msg_channel_id = cmd.source_msg().channel_id;
            if let Err(err) = self.process_command(cmd).await {
                let _ = self
                    .send_message(source_msg_channel_id, |it| err.create_msg(it))
                    .await;
            }
        }
    }

    fn available_track_index_range(&self) -> Option<std::ops::Range<usize>> {
        if self.active_track.is_none() {
            assert_eq!(self.orders.len(), 0);
            None
        } else {
            Some(0..self.orders.len() + 1)
        }
    }

    fn track_index_out_of_bounds_err(&self, index: usize) -> crate::Error {
        crate::err!(TrackIndexOutOfBounds {
            index,
            available: self.available_track_index_range(),
        })
    }

    fn push_track_link(msg: &mut MessageBuilder, order: &AudioTrackOrder) {
        msg.push("[")
            .push_mono_safe(order.meta.title())
            .push("](")
            .push_safe(order.meta.url())
            .push(")");
    }

    fn build_track_status_msg(order: &AudioTrackOrder) -> MessageBuilder {
        let mut msg = MessageBuilder::new();
        msg.push("Track ");
        Self::push_track_link(&mut msg, order);
        msg
    }

    async fn show_track_finished(&self, order: &AudioTrackOrder) -> crate::Result<()> {
        self.send_embed(self.out_channel().await, |it| {
            it.description(
                Self::build_track_status_msg(order)
                    .push("has ")
                    .push_bold("finished")
                    .push(" (played for: ")
                    .push_mono(Self::format_duration(&order.meta.duration()))
                    .push(")"),
            )
        })
        .await
    }

    async fn show_track_removed(
        &self,
        source_msg: &Message,
        index: usize,
        order: &AudioTrackOrder,
    ) -> crate::Result<()> {
        self.send_embed(source_msg.channel_id, |it| {
            it.description(
                Self::build_track_status_msg(&order)
                    .push(" was ")
                    .push_bold(match index {
                        0 => "skipped",
                        _ => "removed from the queue",
                    })
                    .push(" by ")
                    .push_mono_safe(&source_msg.author.name),
            )
        })
        .await?;
        Ok(())
    }

    fn try_add_random_queue_humnail(
        embed: &mut CreateEmbed,
        image: crate::Result<Option<Image>>,
    ) -> &mut CreateEmbed {
        match image {
            Ok(Some(image)) => {
                embed.thumbnail(image.representations.thumb);
            }
            err => warn!(
                ?err,
                "Could not get a random pony image for show queue command"
            ),
        }
        embed
    }

    async fn fetch_random_queue_thumbnail(&self) -> crate::Result<Option<Image>> {
        let tags = ["solo", "face"];
        self.derpibooru
            .fetch_random_media(tags.iter().map(|it| it.parse().unwrap()))
            .await
    }

    async fn process_command(&mut self, cmd: AudioQueueCmd) -> crate::Result<()> {
        match cmd {
            AudioQueueCmd::PlayTrack(order) => {
                if self.active_track.is_some() {
                    let footer = format!(
                        "ordered by {}, time until playing: {}",
                        order.ordered_by.author.name,
                        Self::format_duration(&self.time_until_playing(self.orders.len()).await),
                    );
                    self.send_embed(order.ordered_by.channel_id, |it| {
                        it.title(format_args!("Track pending `#{}`", self.orders.len() + 1))
                            .description(Self::full_yt_video_link(&order.meta))
                            .footer(|it| it.text(footer).icon_url(order.ordered_by.author.face()))
                            .thumbnail(&order.meta.thumbnail_url())
                    })
                    .await?;
                }

                self.orders.push_back(order);
                if self.active_track.is_none() {
                    self.play_next_track().await;
                }
            }
            AudioQueueCmd::SkipTrack { index, source } => {
                if index == 0 {
                    return if let Some(track) = &self.active_track {
                        self.show_track_removed(&source, index, &track.order)
                            .await?;
                        self.play_next_track().await;
                        Ok(())
                    } else {
                        Err(self.track_index_out_of_bounds_err(index))
                    };
                }
                let removed = self
                    .orders
                    .remove(index - 1)
                    .ok_or_else(|| self.track_index_out_of_bounds_err(index))?;

                self.show_track_removed(&source, index, &removed).await?;
            }
            AudioQueueCmd::ShowNowPlaying { source } => {
                if let Some(track) = &self.active_track {
                    self.show_now_playing_track(track).await?;
                } else {
                    self.send_embed(source.channel_id, |it| {
                        it.description(
                            "Nothing is playing right now. Feel free to order your track!",
                        )
                    })
                    .await?;
                }
            }
            AudioQueueCmd::ShowQueue {
                source: Message { channel_id, .. },
            } => {
                let image = self.fetch_random_queue_thumbnail().await;
                let track = match &self.active_track {
                    Some(it) => it,
                    None => {
                        self.send_embed(channel_id, |it| {
                            it.description("Audio queue is empty");
                            Self::try_add_random_queue_humnail(it, image)
                        })
                        .await?;
                        return Ok(());
                    }
                };
                let mut msg = MessageBuilder::new();

                msg.push_bold("Now playing:\n");
                Self::push_track_link(&mut msg, &track.order);
                msg.push_mono_safe(format_args!(
                    "({} / {}) ordered by {}",
                    Self::format_duration(&track.source.lock().await.position),
                    Self::format_duration(&track.order.meta.duration()),
                    track.order.ordered_by.author.name,
                ));

                if !self.orders.is_empty() {
                    msg.push("\n\n").push_bold("In queue:").push("\n");
                }

                for (i, order) in self.orders.iter().enumerate() {
                    msg.push_bold(format_args!("{}. ", i + 1));
                    Self::push_track_link(&mut msg, order);
                    msg.push_mono_line_safe(format_args!(
                        "({}) ordered by {}",
                        Self::format_duration(&order.meta.duration()),
                        order.ordered_by.author.name,
                    ));
                }

                let total_duration = self.time_until_playing(self.orders.len()).await;

                self.send_embed(channel_id, |it| {
                    it.title("Sweetie Bot radio station")
                        .description(msg)
                        .footer(|it| {
                            it.text(format_args!(
                                "Total time left to play: {}",
                                Self::format_duration(&total_duration)
                            ))
                        });
                    Self::try_add_random_queue_humnail(it, image)
                })
                .await?;
            }
            AudioQueueCmd::Pause { source } => {
                let track = self.active_track_or_err()?;
                let mut audio_source = track.source.lock().await;
                if !audio_source.playing {
                    self.send_embed(source.channel_id, |it| {
                        it.description(
                            Self::build_track_status_msg(&track.order)
                                .push(" was already ")
                                .push_bold("paused"),
                        )
                    })
                    .await?;
                } else {
                    audio_source.pause();
                    self.send_embed(source.channel_id, |it| {
                        it.description(
                            Self::build_track_status_msg(&track.order)
                                .push(" was ")
                                .push_bold("paused")
                                .push(" by ")
                                .push_mono_line_safe(&source.author.name),
                        )
                    })
                    .await?;
                }
            }
            AudioQueueCmd::Resume { source } => {
                let track = self.active_track_or_err()?;
                let mut audio_source = track.source.lock().await;
                if audio_source.playing {
                    self.send_embed(source.channel_id, |it| {
                        it.description(
                            Self::build_track_status_msg(&track.order)
                                .push(" was ")
                                .push_bold("not paused"),
                        )
                    })
                    .await?;
                } else {
                    audio_source.play();
                    self.send_embed(source.channel_id, |it| {
                        it.description(
                            Self::build_track_status_msg(&track.order)
                                .push(" was ")
                                .push_bold("resumed")
                                .push(" by ")
                                .push_mono_line_safe(&source.author.name),
                        )
                    })
                    .await?;
                }
            }
            AudioQueueCmd::Clear { source } => {
                if self.orders.is_empty() {
                    self.send_embed(source.channel_id, |it| {
                        it.description("The audio tracks queue is already empty")
                    })
                    .await?;
                }
            }
        }

        Ok(())
    }

    async fn time_until_playing(&self, order_index: usize) -> time::Duration {
        let queue_duration: time::Duration = self
            .orders
            .iter()
            .map(|it| it.meta.duration())
            .take(order_index)
            .sum();

        let track = self.active_track.as_ref().unwrap();
        let active_left = track.order.meta.duration() - track.source.lock().await.position;

        active_left + queue_duration
    }

    fn active_track_or_err(&self) -> crate::Result<&ActiveAudioTrack> {
        match &self.active_track {
            Some(it) => Ok(it),
            None => Err(crate::err!(NoActiveTrack)),
        }
    }

    async fn play_next_track(&mut self) {
        while let Err(err) = self.try_play_next_track().await {
            self.send_message(self.out_channel().await, |it| err.create_msg(it))
                .await
                .unwrap();
        }
    }

    async fn try_play_next_track(&mut self) -> crate::Result<()> {
        if self.active_track.take().is_some() {
            self.voice_mgr
                .lock()
                .await
                .get_mut(&self.guild_id)
                .unwrap()
                .stop();
        }

        let order = match self.orders.pop_front() {
            Some(it) => it,
            None => return Ok(()),
        };

        let guild = self.cache.guild(&self.guild_id).await.unwrap();

        let channel_id = guild
            .voice_states
            .get(&order.ordered_by.author.id)
            .and_then(|it| it.channel_id)
            .ok_or_else(|| crate::err!(UserNotInVoiceChanel))?;

        let channel = guild
            .channels
            .get(&channel_id)
            .expect("BUG: invalid channel id?");

        let mut voice_mgr = self.voice_mgr.lock().await;

        let handler = voice_mgr
            .join(guild.id, channel_id)
            .ok_or_else(|| crate::err!(JoinVoiceChannel(Some(channel.name().to_owned()))))?;

        let source = voice::ytdl(order.meta.url().as_str())
            .await
            .map_err(|err| crate::err!(AudioStart(err)))?;

        let (source, finish_recv) = SubscribableAudioSource::new(source);
        let source = handler.play_only(Box::new(source));

        self.active_track = Some(ActiveAudioTrack {
            order,
            source,
            finish_recv,
        });

        self.show_now_playing_track(self.active_track.as_ref().unwrap())
            .await?;

        Ok(())
    }

    fn full_yt_video_link(yt_vid: &YtVideo) -> MessageBuilder {
        let mut msg = MessageBuilder::new();
        msg.push("[")
            .push_bold_safe(yt_vid.channel_title())
            .push("](")
            .push_safe(yt_vid.channel_url())
            .push(") - [")
            .push_bold_safe(format_args!("\"{}\"", yt_vid.title()))
            .push("](")
            .push_safe(yt_vid.url())
            .push(")");
        msg
    }

    async fn show_now_playing_track(&self, track: &ActiveAudioTrack) -> crate::Result<()> {
        let meta = &track.order.meta;
        let order_msg = &track.order.ordered_by;

        let footer_text = format!(
            "ordered by {} ({} / {})",
            // FIXME: use `.nick_in(guild_id)`
            track.order.ordered_by.author.name,
            Self::format_duration(&track.source.lock().await.position),
            Self::format_duration(&track.order.meta.duration()),
        );
        self.send_embed(order_msg.channel_id, |it| {
            it.title("Now playing")
                .description(Self::full_yt_video_link(meta))
                .thumbnail(&meta.thumbnail_url())
                .footer(|it| it.text(footer_text).icon_url(order_msg.author.face()))
        })
        .await?;
        Ok(())
    }

    /// Returns duration in a colon separated string format.
    fn format_duration(duration: &impl Hhmmss) -> impl fmt::Display {
        // Unfortunately chrono doesn't have anything useful for formatting durations
        // FIXME: use chrono means of formatting durations once this is added to the lib:
        // https://github.com/chronotope/chrono/issues/197#issuecomment-716257398
        let rendered = duration.hhmmss();

        // Remove unnecessary leading zeros for hours (most of the tracks are within the minutes timespan)
        match rendered.strip_prefix("00:") {
            Some(it) => it.to_owned(),
            None => rendered,
        }
    }
}

pub(crate) struct SubscribableAudioSource {
    inner: Box<dyn AudioSource>,
    finish_sender: Option<oneshot::Sender<()>>,
}

impl SubscribableAudioSource {
    pub(crate) fn new(inner: Box<dyn AudioSource>) -> (Self, oneshot::Receiver<()>) {
        let (sender, receiver) = oneshot::channel();
        (
            Self {
                inner,
                finish_sender: Some(sender),
            },
            receiver,
        )
    }

    pub(crate) fn send_finished_event_or_panic(&mut self) {
        // Ignore if the receiver was dropped (it means track was cancelled)
        let _ = self.finish_sender.take().unwrap().send(());
    }
}

#[async_trait]
impl AudioSource for SubscribableAudioSource {
    async fn is_stereo(&mut self) -> bool {
        self.inner.is_stereo().await
    }

    async fn get_type(&self) -> voice::AudioType {
        self.inner.get_type().await
    }

    async fn read_pcm_frame(&mut self, buffer: &mut [i16]) -> Option<usize> {
        let n_read = self.inner.read_pcm_frame(buffer).await;
        // debug!(?n_read);
        if let Some(0) = n_read {
            self.send_finished_event_or_panic();
        }
        n_read
    }

    async fn read_opus_frame(&mut self) -> Option<Vec<u8>> {
        self.inner.read_opus_frame().await
    }

    async fn decode_and_add_opus_frame(
        &mut self,
        float_buffer: &mut [f32; 1920],
        volume: f32,
    ) -> Option<usize> {
        self.inner
            .decode_and_add_opus_frame(float_buffer, volume)
            .await
    }
}
