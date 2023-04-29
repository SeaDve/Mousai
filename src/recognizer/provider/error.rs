use gettextrs::gettext;
use gtk::glib;
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, glib::Boxed)]
#[boxed_type(name = "MsaiRecognizeError")]
pub struct RecognizeError {
    kind: RecognizeErrorKind,
    message: Option<String>,
}

impl error::Error for RecognizeError {}

impl fmt::Display for RecognizeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(message) = self.message() {
            write!(f, "{}: {}", self.title(), message)
        } else {
            write!(f, "{}", self.title())
        }
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

    pub fn title(&self) -> String {
        match self.kind() {
            RecognizeErrorKind::NoMatches => gettext("No Matches Found"),
            RecognizeErrorKind::Fingerprint => gettext("Cannot Create Fingerprint From Audio"),
            RecognizeErrorKind::InvalidToken => gettext("Invalid Token Given"),
            RecognizeErrorKind::TokenLimitReached => gettext("Token Limit Reached"),
            RecognizeErrorKind::Connection => gettext("Cannot Connect to the Server"),
            RecognizeErrorKind::OtherPermanent => gettext("Received Other Permanent Error"),
        }
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
