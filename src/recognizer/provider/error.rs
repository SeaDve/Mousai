use gettextrs::gettext;

use std::{error, fmt};

#[derive(Debug)]
pub struct NoMatchesError;

impl fmt::Display for NoMatchesError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&gettext("No matches found for this song"))
    }
}

impl error::Error for NoMatchesError {}

#[derive(Debug)]
pub struct FingerprintError;

impl fmt::Display for FingerprintError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&gettext("Failed to create fingerprint from audio"))
    }
}

impl error::Error for FingerprintError {}

#[derive(Debug)]

pub struct ResponseParseError;

impl fmt::Display for ResponseParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&gettext("Failed to parse response from provider"))
    }
}

impl error::Error for ResponseParseError {}

#[derive(Debug)]
pub enum TokenError {
    Invalid,
    LimitReached,
}

impl fmt::Display for TokenError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TokenError::Invalid => f.write_str(&gettext("Invalid token")),
            TokenError::LimitReached => f.write_str(&gettext("Token limit reached")),
        }
    }
}

impl error::Error for TokenError {}
