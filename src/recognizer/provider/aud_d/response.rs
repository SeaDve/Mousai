use serde::Deserialize;

use crate::recognizer::provider::error::{RecognizeError, RecognizeErrorKind};

#[derive(Debug, Deserialize)]
pub struct LyricsData {
    pub lyrics: String,
    /// A json object containing `provider` and `url` field
    pub media: String,
}

#[derive(Debug, Deserialize)]
pub struct Preview {
    pub url: String,
}

#[derive(Debug, Deserialize)]
pub struct Artwork {
    pub url: String,
}

#[derive(Debug, Deserialize)]
pub struct AppleMusicData {
    pub previews: Vec<Preview>,
    pub url: String,
    pub artwork: Artwork,
}

#[derive(Debug, Deserialize)]
pub struct Image {
    pub url: String,
}

#[derive(Debug, Deserialize)]
pub struct Album {
    pub images: Vec<Image>,
}

#[derive(Debug, Deserialize)]
pub struct ExternalUrls {
    pub spotify: String,
}

#[derive(Debug, Deserialize)]
pub struct SpotifyData {
    pub album: Album,
    pub disc_number: u32,
    pub track_number: u32,
    pub preview_url: String,
    pub external_urls: ExternalUrls,
}

#[derive(Debug, Deserialize)]
pub struct Data {
    pub artist: String,
    pub title: String,
    pub album: String,
    /// In format of ISO-8601 (%Y-%m-%d)
    pub release_date: Option<String>,
    // TODO consider showing this in the ui
    pub label: Option<String>,
    pub timecode: String,
    #[serde(rename(deserialize = "song_link"))]
    pub info_link: String,
    #[serde(rename(deserialize = "spotify"))]
    pub spotify_data: Option<SpotifyData>,
    #[serde(rename(deserialize = "apple_music"))]
    pub apple_music_data: Option<AppleMusicData>,
    #[serde(rename(deserialize = "lyrics"))]
    pub lyrics_data: Option<LyricsData>,
}

#[derive(Debug, Deserialize)]
struct AudDRawError {
    #[serde(rename(deserialize = "error_code"))]
    code: u16,
    #[serde(rename(deserialize = "error_message"))]
    message: String,
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
    pub fn data(self) -> Result<Data, RecognizeError> {
        if self.status == "success" {
            return self
                .data
                .ok_or_else(|| RecognizeError::new(RecognizeErrorKind::NoMatches, None));
        }

        if self.status == "error" {
            let error = self.error.ok_or_else(|| {
                RecognizeError::new(
                    RecognizeErrorKind::OtherPermanent,
                    "Got `error` status but no error".to_string(),
                )
            })?;

            // Based on https://docs.audd.io/#common-errors
            let kind = match error.code {
                901 => RecognizeErrorKind::TokenLimitReached,
                900 => RecognizeErrorKind::InvalidToken,
                300 => RecognizeErrorKind::Fingerprint,
                _ => RecognizeErrorKind::OtherPermanent,
            };
            return Err(RecognizeError::new(
                kind,
                format!("#{}: {}", error.code, error.message),
            ));
        }

        Err(RecognizeError::new(
            RecognizeErrorKind::OtherPermanent,
            format!("Got invalid status response of {}", self.status),
        ))
    }
}
