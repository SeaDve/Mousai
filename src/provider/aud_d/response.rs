use serde::Deserialize;

use super::error::{AudDError, Error};

#[derive(Debug, Deserialize)]
pub struct Image {
    pub url: String,
}

#[derive(Debug, Deserialize)]
pub struct Album {
    pub images: Vec<Image>,
}

#[derive(Debug, Deserialize)]
pub struct SpotifyData {
    pub album: Album,
    pub disc_number: u32,
    pub track_number: u32,
    pub preview_url: String,
}

#[derive(Debug, Deserialize)]
pub struct Data {
    pub artist: String,
    pub title: String,
    pub album: String,
    /// In format of ISO-8601 (%Y-%m-%d)
    pub release_date: String,
    // TODO consider showing this in the ui
    pub label: String,
    pub timecode: String,
    #[serde(rename(deserialize = "song_link"))]
    pub info_link: String,
    #[serde(rename(deserialize = "spotify"))]
    pub spotify_data: Option<SpotifyData>,
}

#[derive(Debug, Deserialize)]
pub struct AudDRawError {
    #[serde(rename(deserialize = "error_code"))]
    pub code: u16,
    #[serde(rename(deserialize = "error_message"))]
    pub message: String,
}

impl AudDRawError {
    pub fn into_aud_d_error(self) -> AudDError {
        match self.code {
            901 => AudDError::DailyLimitReached,
            900 => AudDError::InvalidToken,
            300 => AudDError::Fingerprint(self.message),
            other_code => AudDError::Other(format!("{} ({})", self.message, other_code)),
        }
    }
}

/// If `status` is `success` `data` is `Some` and `error` is `None`. On the other hand, if status is
/// `error` it is the opposite.
///
/// Thus `data` and `error` are mutually exclusive.
#[derive(Debug, Deserialize)]
pub struct Response {
    status: String,
    #[serde(rename(deserialize = "result"))]
    data: Option<Data>,
    error: Option<AudDRawError>,
}

impl Response {
    pub fn parse(slice: &[u8]) -> Result<Self, Error> {
        Ok(serde_json::from_slice(slice)?)
    }

    pub fn data(self) -> Result<Data, AudDError> {
        match self.status.as_str() {
            "success" => self.data.map_or_else(|| Err(AudDError::NoMatches), Ok),
            "error" => Err(self.error.map_or_else(
                || AudDError::Other("Got `error` status, but no error".into()),
                |error| error.into_aud_d_error(),
            )),
            other => Err(AudDError::Other(format!(
                "Got invalid status response of {}",
                other
            ))),
        }
    }
}
