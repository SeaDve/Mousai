use gettextrs::gettext;
use gtk::{gio, glib};

use super::ProviderError;

#[derive(Debug)]
pub enum AudDError {
    /// Song inputted has no matches
    NoMatches,
    /// Passed in invalid API token
    InvalidToken,
    /// There is no API token passed, and the daily limit was reached.
    DailyLimitReached,
    /// There was a problem with audio decoding or with the neural network. Possibly, the audio file is too small.
    Fingerprint(String),
    /// Miscellaneous errors
    Other(String),
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Problem in HTTPS response parsing
    #[error("Failed to parse response: {0}")]
    Parse(#[from] serde_json::Error),

    /// Problem in converting the AudioRecording to a recognizable format
    #[error("Failed to convert AudioRecording to recognizable format: {0}")]
    Convert(glib::Error),

    /// Request specific errors
    #[error("Failed to create request: {0}")]
    Soup(glib::Error),

    /// AudD specific errors
    #[error("AudD specific error: {0:?}")]
    AudD(AudDError),

    #[error("{0}")]
    Other(String),
}

impl From<AudDError> for Error {
    fn from(err: AudDError) -> Self {
        Self::AudD(err)
    }
}

impl From<Error> for ProviderError {
    fn from(this: Error) -> Self {
        match this {
            Error::Parse(_) => ProviderError::Other(gettext(
                "Failed to parse response. Please report this to Mousai's bug tracker.",
            )),
            Error::Convert(_) => ProviderError::Other(gettext(
                "Failed to convert file. Please report this to Mousai's bug tracker.",
            )),
            Error::Soup(err) => {
                if matches!(
                    err.kind::<gio::ResolverError>(),
                    Some(gio::ResolverError::TemporaryFailure)
                ) {
                    ProviderError::Connection(gettext("Failed to connect to the server."))
                } else {
                    ProviderError::Connection(err.to_string())
                }
            }
            Error::AudD(err) => match err {
                AudDError::NoMatches => ProviderError::NoMatches,
                AudDError::InvalidToken => ProviderError::InvalidToken,
                AudDError::DailyLimitReached => {
                    ProviderError::NoToken(gettext("Daily limit has been reached."))
                }
                AudDError::Fingerprint(_) => ProviderError::Other(gettext(
                    "Failed to fingerprint audio. There may be no sound heard.",
                )),
                AudDError::Other(other) => ProviderError::Other(other),
            },
            Error::Other(err) => ProviderError::Other(err),
        }
    }
}
