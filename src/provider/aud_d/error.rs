use gtk::glib;

#[derive(Debug)]
pub enum AudDError {
    // Song inputted has no matches
    NoMatches,
    // Passed in invalid API token
    InvalidToken,
    // There is no API token passed, and the daily limit was reached.
    DailyLimitReached,
    // There was a problem with audio decoding or with the neural network. Possibly, the audio file is too small.
    Fingerprint(String),
    // Miscellaneous errors
    Other(String),
}

#[derive(Debug)]
pub enum Error {
    // Problem in HTTPS response parsing
    Parse(serde_json::Error),
    // Problem converting file into base64
    FileConvert(glib::Error),
    // Request sepecific errors
    Reqwest(reqwest::Error),
    // AudD specific errors
    AudD(AudDError),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Parse(error) => write!(f, "Failed to parse response: {:?}", error),
            Self::FileConvert(error) => write!(f, "Failed to convert file to base 64 {}", error),
            Self::Reqwest(error) => write!(f, "Failed to create request: {}", error),
            Self::AudD(error) => write!(f, "AudD Specific error: {:?}", error),
        }
    }
}

impl std::error::Error for Error {}

impl From<serde_json::Error> for Error {
    fn from(err: serde_json::Error) -> Self {
        Self::Parse(err)
    }
}

impl From<reqwest::Error> for Error {
    fn from(err: reqwest::Error) -> Self {
        Self::Reqwest(err)
    }
}

impl From<AudDError> for Error {
    fn from(err: AudDError) -> Self {
        Self::AudD(err)
    }
}
