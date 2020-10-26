//! Audio tracks queue implementation

use crate::yt::YtVideo;
use serenity::{model::prelude::User, prelude::Mutex, voice::Audio};
use std::sync::Arc;

pub(crate) struct AudioTrack {
    pub(crate) meta: YtVideo,
    pub(crate) source: Arc<Mutex<Audio>>,
    pub(crate) ordered_by: User,
}

// TODO: add an audio tracks queue, we will use it to track the state of
// currently scheduled and active audio tracks
// It will be injected via the DI system.
