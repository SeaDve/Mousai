use gettextrs::gettext;
use serde::{Deserialize, Serialize};

use std::{error, fmt};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RecognizeErrorKind {
    NoMatches,
    Fingerprint,
    InvalidToken,
    TokenLimitReached,
    Connection,
    OtherPermanent,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecognizeError {
    kind: RecognizeErrorKind,
    message: Option<String>,
}

impl error::Error for RecognizeError {}

impl fmt::Display for RecognizeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&match self.kind() {
            RecognizeErrorKind::NoMatches => gettext("No matches found for this song"),
            RecognizeErrorKind::Fingerprint => gettext("Failed to create fingerprint from audio"),
            RecognizeErrorKind::InvalidToken => gettext("Invalid token given"),
            RecognizeErrorKind::TokenLimitReached => gettext("Token limit reached"),
            RecognizeErrorKind::Connection => gettext("Failed to connect to the server"),
            RecognizeErrorKind::OtherPermanent => gettext("Permanent error received"),
        })
    }
}

impl RecognizeError {
    pub fn new(kind: RecognizeErrorKind, message: impl Into<Option<String>>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }

    pub fn kind(&self) -> RecognizeErrorKind {
        self.kind
    }

    pub fn message(&self) -> Option<&str> {
        self.message.as_deref()
    }

    /// Whether the failure is permanent (i.e. "no matches found for
    /// this recording", in contrast to "internet connection error" or
    /// "expired token error")
    ///
    /// Permanent failures are not retried because they are unlikely to
    /// be resolved by retrying.
    pub fn is_permanent(&self) -> bool {
        use RecognizeErrorKind::*;

        match self.kind() {
            NoMatches | Fingerprint | OtherPermanent => true,
            Connection | TokenLimitReached | InvalidToken => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serde_bincode() {
        let val = RecognizeError::new(RecognizeErrorKind::Connection, None);
        let bytes = bincode::serialize(&val).unwrap();
        let de_val = bincode::deserialize(&bytes).unwrap();
        assert_eq!(val, de_val);

        let val = RecognizeError::new(
            RecognizeErrorKind::Connection,
            "Some error message".to_string(),
        );
        let bytes = bincode::serialize(&val).unwrap();
        let de_val = bincode::deserialize(&bytes).unwrap();
        assert_eq!(val, de_val);
    }
}
