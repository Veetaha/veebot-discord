//! Symbols related to communicating with the Gelbooru API

use crate::util::{self, ReqwestBuilderExt, ThemeTag};
use itertools::Itertools;
use std::{collections::HashSet, iter, sync::Arc};
use url::Url;

/// Declarations of the gelbooru JSON API types.
pub(crate) mod rpc {
    use super::*;
    use serde::Deserialize;

    pub(crate) mod search {
        use super::*;

        #[derive(Debug, Deserialize)]
        pub(crate) struct Response(pub(crate) Vec<Image>);
    }

    #[derive(Debug, Deserialize)]
    pub(crate) struct Image {
        pub(crate) id: u128,
        pub(crate) file_url: Url,
        pub(crate) tags: String,
        // FIXME: properly parse the date
        pub(crate) created_at: String,
        pub(crate) score: u64,
    }
}

util::def_url_base!(gelbooru_api, "https://gelbooru.com/index.php");
util::def_url_base!(gelbooru, "https://gelbooru.com/index.php");

impl rpc::Image {
    pub(crate) fn webpage_url(&self) -> Url {
        let mut url = gelbooru(iter::empty::<&str>());
        url.query_pairs_mut().extend_pairs(&[
            ("page", "post"),
            ("s", "view"),
            ("id", &self.id.to_string()),
        ]);
        url
    }
}

pub(crate) struct GelbooruService {
    http_client: Arc<reqwest::Client>,
    gelbooru_api_key: String,
    gelbooru_user_id: String,
}

impl GelbooruService {
    pub(crate) fn new(
        gelbooru_api_key: String,
        gelbooru_user_id: String,
        http_client: Arc<reqwest::Client>,
    ) -> Self {
        Self {
            http_client,
            gelbooru_api_key,
            gelbooru_user_id,
        }
    }

    pub(crate) async fn fetch_random_media(
        &self,
        tags: impl IntoIterator<Item = ThemeTag>,
    ) -> crate::Result<Option<rpc::Image>> {
        let mut tags = tags.into_iter().collect::<HashSet<_>>();

        if !tags.iter().any(|it| it.as_str().starts_with("sort:")) {
            tags.insert("sort:random".parse().unwrap());
        }

        let tags = tags.iter().join(" ");

        let query = vec![
            ("page", "dapi"),
            ("s", "post"),
            ("q", "index"),
            ("limit", "1"),
            ("json", "1"),
            ("tags", &tags),
            ("api_key", &self.gelbooru_api_key),
            ("user_id", &self.gelbooru_user_id),
        ];

        let res: crate::Result<rpc::search::Response> = self
            .http_client
            .get(gelbooru_api(iter::empty::<&str>()))
            .query(&query)
            .read_json()
            .await;

        match res {
            Ok(it) => Ok(it.0.into_iter().next()),
            // When no image was found, the response has empty body...
            Err(crate::Error {
                kind: crate::ErrorKind::UnexpectedHttpResponseJsonShape(_),
                ..
            }) => Ok(None),
            Err(it) => Err(it),
        }
    }
}
