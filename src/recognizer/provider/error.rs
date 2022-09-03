use gettextrs::gettext;

use std::fmt;

#[derive(Debug, PartialEq, Eq)]
pub enum ProviderError {
    NoMatches,
    NoToken(String),
    InvalidToken,
    Connection(String),
    Other(String),
}

impl fmt::Display for ProviderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ProviderError::Connection(string) => {
                f.write_str(&gettext!("{} Check your internet connection.", string))
            }
            ProviderError::NoMatches => f.write_str(&gettext("No matches found for this song.")),
            ProviderError::NoToken(string) => f.write_str(&gettext!(
                "{} Input an API token in the preferences.",
                string
            )),
            ProviderError::InvalidToken => f.write_str(&gettext("Please input a valid API token.")),
            ProviderError::Other(string) => f.write_str(string),
        }
    }
}

impl std::error::Error for ProviderError {}
