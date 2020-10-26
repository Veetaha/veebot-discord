use serenity::{builder::CreateMessage, framework::standard::ArgError, utils::Color};
use thiserror::Error;
use url::Url;
// We have to rename it because `thiserror` implements
// the unstable `std::Error::backtrace()` if the error contains a field of `some_crate::Backtrace` type.
use backtrace::Backtrace as BacktracePolyfill;

pub type Result<T, E = Error> = std::result::Result<T, E>;

/// Small macro to reduce the boilerplate for instantiating application errors.
macro_rules! _err {
    ($variant:ident $($args:tt)*) => {{
        $crate::error::Error::from($crate::error::ErrorKind::$variant $($args)*)
    }};
}

pub(crate) use _err as err;

#[derive(Debug, Error)]
#[error("{kind}")]
pub struct Error {
    /// Small identifier used for debugging purposes.
    /// It is mentioned in the chat when the error happens.
    /// This way we as developers can copy it and lookup the logs using this id.
    pub(crate) id: String,
    pub(crate) backtrace: BacktracePolyfill,
    pub(crate) kind: ErrorKind,
}

impl<T: Into<ErrorKind>> From<T> for Error {
    fn from(kind: T) -> Self {
        let err = Self {
            kind: kind.into(),
            id: nanoid::nanoid!(6),
            backtrace: BacktracePolyfill::new(),
        };

        tracing::error!(id = err.id.as_str(), ?err.backtrace, ?err.kind);

        err
    }
}

impl Error {
    /// Method used by code generated via [`veebot_cmd::veebot_cmd`] proc macro.
    /// If the command handler returns an [`Err`] result, this method will be
    /// invoked to create a message to be sent to the chat the command came from
    /// just to show users some information about what went wrong.
    pub(crate) fn create_msg<'a, 'b>(
        &self,
        msg: &'a mut CreateMessage<'b>,
    ) -> &'a mut CreateMessage<'b> {
        msg
            .embed(|it| it
                .title(self.kind.title())
                .description(format_args!(
                    "{}\n\nError id: **`{}`**\n\n```\n{:#?}\n```",
                    self,
                    self.id,
                    self.kind,
                ))
                .color(Color::from_rgb(167, 14, 37))
                .thumbnail("https://images-wixmp-ed30a86b8c4ca887773594c2.wixmp.com/f/f317c91e-d216-4cb4-92ad-a690a1792fba/d4qcyp5-ccd57935-27b2-4dcd-8da3-8796865be522.png/v1/fill/w_206,h_250,strp/i_just_don_t_know_what_went_wrong_by_toxickittycat_d4qcyp5-250t.png?token=eyJ0eXAiOiJKV1QiLCJhbGciOiJIUzI1NiJ9.eyJzdWIiOiJ1cm46YXBwOjdlMGQxODg5ODIyNjQzNzNhNWYwZDQxNWVhMGQyNmUwIiwiaXNzIjoidXJuOmFwcDo3ZTBkMTg4OTgyMjY0MzczYTVmMGQ0MTVlYTBkMjZlMCIsIm9iaiI6W1t7ImhlaWdodCI6Ijw9OTg2IiwicGF0aCI6IlwvZlwvZjMxN2M5MWUtZDIxNi00Y2I0LTkyYWQtYTY5MGExNzkyZmJhXC9kNHFjeXA1LWNjZDU3OTM1LTI3YjItNGRjZC04ZGEzLTg3OTY4NjViZTUyMi5wbmciLCJ3aWR0aCI6Ijw9ODExIn1dXSwiYXVkIjpbInVybjpzZXJ2aWNlOmltYWdlLm9wZXJhdGlvbnMiXX0.jgu-An5VgiWIhLEUxo5u1pKujheBDx09mtmN7AwDFKU")
            )
    }
}

#[derive(Error, Debug)]
pub enum ErrorKind {
    #[error("You are not in a discord server (guild) right now")]
    UserNotInGuild,

    #[error("Failed to parse url: {0}")]
    UrlArgParse(#[from] ArgError<url::ParseError>),

    #[error(
        "You are not in a voice channel. You need to connect to one first so that \
        I can understand which channel to join."
    )]
    UserNotInVoiceChanel,

    #[error("I cannot join the voice channel {}", .0.as_deref().unwrap_or("<unknown channel name>"))]
    JoinVoiceChannel(Option<String>),

    #[error("Falied to start streaming the audio: {0}")]
    AudioStart(serenity::Error),

    #[error("Unknown discord error: {0}")]
    UnknownDiscord(#[from] serenity::Error),

    #[error("Failed to send an http request")]
    SendRequest(reqwest::Error),

    #[error("YouTube has returned an error (http status code: {status}) {err}")]
    YtBadStatusCode {
        status: reqwest::StatusCode,
        err: String,
    },

    #[error("YouTube has returned an unexpected response JSON obejct")]
    YtUnexpectedJsonShape(reqwest::Error),

    #[error("Failed to find youtube video for \"{0}\" query.)")]
    YtVidNotFound(String),

    #[error("Could not infer YouTube video id from the url `{0}`")]
    YtInferVideoId(Url),
}

impl ErrorKind {
    /// Short name of the kind of this error.
    fn title(&self) -> &'static str {
        match self {
            ErrorKind::UserNotInGuild => "Not in a guild error",
            ErrorKind::UrlArgParse(_) => "Invalid argument error",
            ErrorKind::UserNotInVoiceChanel => "Not in a voice channel error",
            ErrorKind::JoinVoiceChannel(_) => "Permissions error",
            ErrorKind::AudioStart(_) | ErrorKind::UnknownDiscord(_) => "Internal error",
            ErrorKind::SendRequest(_) => "Send request erorr",
            ErrorKind::YtBadStatusCode { .. }
            | ErrorKind::YtUnexpectedJsonShape(_)
            | ErrorKind::YtVidNotFound(_) => "YouTube error",
            ErrorKind::YtInferVideoId { .. } => "Bad YouTube URL",
        }
    }
}
