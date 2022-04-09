use gtk::glib;

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

    /// Problem converting file into base64
    #[error("Failed to convert file to base 64: {0}")]
    FileConvert(glib::Error),

    /// Request sepecific errors
    #[error("Failed to create request: {0}")]
    Reqwest(#[from] reqwest::Error),

    /// AudD specific errors
    #[error("AudD specific error: {0:?}")]
    AudD(AudDError),
}

impl From<AudDError> for Error {
    fn from(err: AudDError) -> Self {
        Self::AudD(err)
    }
}
