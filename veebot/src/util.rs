//! Assorted utility functions (missing batteries).

use std::{fmt, str::FromStr, time};

use bytes::Bytes;
use hhmmss::Hhmmss;
use serde::de::DeserializeOwned;
use serenity::{
    async_trait,
    model::{guild::Guild, id::GuildId},
};
use tracing::{debug, warn};

/// Shortcut for defining a lazily-compiled regular expression
macro_rules! _regex {
    ($regex_body:literal) => {{
        static RE: ::once_cell::sync::OnceCell<regex::Regex> = ::once_cell::sync::OnceCell::new();
        RE.get_or_init(|| ::regex::Regex::new($regex_body).unwrap())
    }};
}

macro_rules! _def_url_base {
    ($ident:ident, $url:literal) => {
        fn $ident<T: AsRef<str>>(segments: impl IntoIterator<Item = T>) -> ::url::Url {
            let mut url: ::url::Url = $url.parse().unwrap();
            url.path_segments_mut().unwrap().extend(segments);
            url
        }
    };
}

pub(crate) use {_def_url_base as def_url_base, _regex as regex};

#[async_trait]
pub(crate) trait CacheExt {
    async fn guild_or_err(&self, guild_id: GuildId) -> crate::Result<Guild>;
}

#[async_trait]
impl CacheExt for serenity::client::Cache {
    async fn guild_or_err(&self, guild_id: GuildId) -> crate::Result<Guild> {
        self.guild(guild_id)
            .await
            .ok_or_else(|| crate::err!(DiscordGuildCacheMiss(guild_id)))
    }
}

#[async_trait]
pub(crate) trait ReqwestBuilderExt {
    async fn read_json<T: DeserializeOwned>(
        self,
        // url: Url,
        // query: &[(&str, &str)],
    ) -> crate::Result<T>;

    async fn read_bytes(self) -> crate::Result<Bytes>;
}

#[async_trait]
impl ReqwestBuilderExt for reqwest::RequestBuilder {
    async fn read_json<T: DeserializeOwned>(self) -> crate::Result<T> {
        let bytes = self.read_bytes().await?;

        serde_json::from_slice(&bytes).map_err(|err| {
            match std::str::from_utf8(&bytes) {
                Ok(response_body) => warn!(response_body, "Bad JSON response"),
                Err(utf8_decode_err) => warn!(
                    response_body = ?bytes,
                    ?utf8_decode_err,
                    "Bad JSON response"
                ),
            };
            crate::err!(UnexpectedHttpResponseJsonShape(err))
        })
    }

    async fn read_bytes(self) -> crate::error::Result<Bytes> {
        debug!(request = ?self, "sending HTTP request");

        let res = self
            // XXX: important for derpibooru (otherwise it responds with an html capcha page)
            .header("User-Agent", "Veebot")
            .send()
            .await
            .map_err(|err| crate::err!(SendHttpRequest(err)))?;

        let status = res.status();

        if status.is_client_error() || status.is_server_error() {
            let body = match res.text().await {
                Ok(it) => it,
                Err(err) => format!("Could not collect the error response body text: {}", err),
            };

            return Err(crate::err!(BadHttpResponseStatusCode { status, body }));
        }

        res.bytes()
            .await
            .map_err(|err| crate::err!(ReadHttpResponse(err)))
    }
}

pub(crate) fn create_http_client() -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(time::Duration::from_secs(30))
        .connect_timeout(time::Duration::from_secs(30))
        .build()
        .expect("rustls backend initialization should never error out")
}

// A string without commas
#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub(crate) struct ThemeTag(String);

impl ThemeTag {
    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ThemeTag {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.0, f)
    }
}

impl FromStr for ThemeTag {
    type Err = crate::Error;

    fn from_str(s: &str) -> Result<ThemeTag, Self::Err> {
        let input = s.to_owned();
        if s.contains(',') {
            return Err(crate::err!(CommaInImageTag { input }));
        }
        Ok(ThemeTag(input))
    }
}

/// Returns duration in a colon separated string format.
pub(crate) fn format_duration(duration: &impl Hhmmss) -> String {
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
