//! Symbols related to communicating with the YouTube API

use std::{fmt, sync::Arc, time};
use url::Url;
use util::{regex, ReqwestBuilderExt};

use crate::util;

/// Declarations of the types used in the YouTube's API.
/// We could've used some crate that defines them, but
/// we avoid such a dependency (also taking into account that
/// the ones @Veetaha has found at the time of this writing are
/// quite outdated and unmaintained).
mod rpc {
    use serde::Deserialize;
    use url::Url;

    pub(crate) mod search {
        use serde::Deserialize;

        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        pub(crate) struct Response {
            pub(crate) items: Vec<Item>,
        }

        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        pub(crate) struct Item {
            pub(crate) id: VideoId,
        }

        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        pub(crate) struct VideoId {
            pub(crate) video_id: String,
        }
    }

    pub(crate) mod videos {
        use super::{ContentDetails, LiveStreamingDetails, VideoSnippet};
        use serde::Deserialize;

        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        pub(crate) struct Response {
            pub(crate) items: Vec<Item>,
        }

        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        pub(crate) struct Item {
            pub(crate) id: String,
            pub(crate) snippet: VideoSnippet,
            pub(crate) content_details: ContentDetails,
            pub(crate) live_streaming_details: Option<LiveStreamingDetails>,
        }
    }

    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub(crate) struct VideoSnippet {
        pub(crate) channel_id: String,
        pub(crate) channel_title: String,
        pub(crate) title: String,
        pub(crate) thumbnails: VideoThumbnails,
        // "publishedAt": datetime,
        // "description": string,
    }

    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub(crate) struct VideoThumbnails {
        pub(crate) default: VideoThumbnail,
    }

    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub(crate) struct VideoThumbnail {
        pub(crate) url: Url,
    }

    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub(crate) struct ContentDetails {
        pub(crate) duration: String,
    }

    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub(crate) struct LiveStreamingDetails {}
}

util::def_url_base!(yt_api, "https://www.googleapis.com/youtube/v3");
util::def_url_base!(yt, "https://www.youtube.com");

pub(crate) struct YtVideo(rpc::videos::Item);

impl YtVideo {
    pub(crate) fn is_livestream(&self) -> bool {
        self.0.live_streaming_details.is_some()
    }

    pub(crate) fn url(&self) -> Url {
        let mut url = yt(&["watch"]);
        url.query_pairs_mut().append_pair("v", &self.0.id);
        url
    }

    pub(crate) fn format_duration(&self) -> impl fmt::Display {
        let duration = self.duration();
        if self.is_livestream() && duration == time::Duration::new(0, 0) {
            "livestream".to_owned()
        } else {
            crate::util::format_duration(&duration)
        }
    }

    pub(crate) fn duration(&self) -> time::Duration {
        // FIXME: unfortunately chrono doesn't have parsing of duration from iso8601
        // create an upstream issue about that?
        let dur = iso8601::duration(&self.0.content_details.duration).unwrap();

        time::Duration::from_secs_f64(match dur {
            iso8601::Duration::YMDHMS {
                year,
                month,
                day,
                hour,
                minute,
                second,
                millisecond: _,
            } => {
                f64::from(year) * 60. * 60. * 24. * 30. * 12.
                    + f64::from(month) * 60. * 60. * 24. * 30.
                    + f64::from(day) * 60. * 60. * 24.
                    + f64::from(hour) * 60. * 60.
                    + f64::from(minute) * 60.
                    + f64::from(second)
            }
            iso8601::Duration::Weeks(_) => todo!(),
        })
    }

    pub(crate) fn thumbnail_url(&self) -> &Url {
        &self.0.snippet.thumbnails.default.url
    }

    pub(crate) fn channel_title(&self) -> &str {
        &self.0.snippet.channel_title
    }

    pub(crate) fn title(&self) -> &str {
        &self.0.snippet.title
    }

    pub(crate) fn channel_url(&self) -> Url {
        yt(&["channel", &self.0.snippet.channel_id])
    }
}

pub(crate) struct YtService {
    http_client: Arc<reqwest::Client>,
    yt_data_api_key: String,
}

impl YtService {
    pub(crate) fn new(yt_data_api_key: String, http_client: Arc<reqwest::Client>) -> Self {
        Self {
            yt_data_api_key,
            http_client,
        }
    }

    /// https://developers.google.com/youtube/v3/docs/videos/list
    async fn find_video_by_id(&self, id: &str) -> crate::Result<Option<YtVideo>> {
        let res: rpc::videos::Response = self
            .http_client
            .get(yt_api(&["videos"]))
            .query(&[
                ("part", "snippet,contentDetails,liveStreamingDetails"),
                ("id", id),
                ("key", &self.yt_data_api_key),
            ])
            .read_json()
            .await?;

        Ok(res.items.into_iter().next().map(YtVideo))
    }

    pub(crate) async fn find_video_by_url(&self, url: &Url) -> crate::Result<YtVideo> {
        self.find_video_by_id(&Self::video_id_from_url(url)?)
            .await?
            .ok_or_else(|| crate::err!(YtVidNotFound(url.as_str().to_owned())))
    }

    /// Tries to find yotube video by given `query` string.
    /// Returns `None` or first found youtube video id.b
    /// Search query to find video for.
    /// See: https://developers.google.com/youtube/v3/docs/search/list?apix_params=%7B%22part%22%3A%22snippet%22%2C%22relatedToVideoId%22%3A%22Ks-_Mh1QhMc%22%2C%22type%22%3A%22video%22%7D#usage
    pub(crate) async fn find_video_by_query(&self, query: &str) -> crate::Result<YtVideo> {
        // First perform a search with the given human query string
        let res: rpc::search::Response = self
            .http_client
            .get(yt_api(&["search"]))
            .query(&[
                ("maxResults", "1"),
                ("type", "video"),
                ("q", query),
                ("key", &self.yt_data_api_key),
            ])
            .read_json()
            .await?;

        // Take the returned video metadata (if it was found)
        let video = res
            .items
            .into_iter()
            .next()
            .ok_or_else(|| crate::err!(YtVidNotFound(query.to_owned())))?;

        // Do one more query (this one allows us to get extended info)
        self.find_video_by_id(&video.id.video_id)
            .await?
            .ok_or_else(|| crate::err!(YtVidNotFound(query.to_owned())))
    }

    /// Ported code from JavaScript `ytdl-core` library:
    /// https://github.com/fent/node-ytdl-core/blob/20a18e5cc93fc7ea76607b33a4f6061cf7e96014/lib/util.js#L238-L309
    fn video_id_from_url(url: &Url) -> crate::Result<String> {
        let valid_path_domains_regex = regex! {
            r#"^https?://(?:youtu\.be/|(?:www\.)?youtube\.com/(?:embed|v)/)"#
        };

        let id = url
            .query_pairs()
            .find(|(key, _)| key == "v")
            .map(|(_, val)| val);

        let id = match (valid_path_domains_regex.is_match(url.as_str()), id) {
            (true, None) => url
                .path_segments()
                .expect("BUG: url matched the valid path domains regex is not cannot-be-a-base")
                .rev()
                .next()
                .expect("BUG: there has to be at least one path segment (even empty)")
                .to_owned(),

            (_, Some(id)) if url.host_str().map(is_valid_query_domain) == Some(true) => {
                id.into_owned()
            }
            _ => return Err(crate::err!(YtInferVideoId(url.to_owned()))),
        };

        let id_regex = regex! {
            r#"^[a-zA-Z0-9-_]{11}$"#
        };

        return if id_regex.is_match(&id) {
            Ok(id)
        } else {
            Err(crate::err!(YtInferVideoId(url.to_owned())))
        };

        fn is_valid_query_domain(domain: &str) -> bool {
            matches!(
                domain,
                "youtube.com"
                    | "www.youtube.com"
                    | "m.youtube.com"
                    | "music.youtube.com"
                    | "gaming.youtube.com"
            )
        }
    }
}
