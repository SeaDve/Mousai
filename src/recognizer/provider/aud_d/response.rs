use anyhow::{anyhow, Context, Result};
use gettextrs::gettext;
use serde::Deserialize;

use crate::recognizer::provider::error::{FingerprintError, NoMatchesError, TokenError};

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
pub struct AudDRawError {
    #[serde(rename(deserialize = "error_code"))]
    pub code: u16,
    #[serde(rename(deserialize = "error_message"))]
    pub message: String,
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
    pub fn parse(slice: &[u8]) -> Result<Self> {
        serde_json::from_slice(slice).context("Failed to parse AudD response")
    }

    pub fn data(self) -> Result<Data> {
        if self.status == "success" {
            return self.data.ok_or_else(|| NoMatchesError.into());
        }

        if self.status == "error" {
            let error = self
                .error
                .ok_or_else(|| anyhow!("Got `error` status but no error"))?;

            // Based on https://docs.audd.io/#common-errors

            let err = anyhow!("#{}: {}", error.code, error.message);
            return Err(match error.code {
                901 => err.context(TokenError::LimitReached),
                900 => err.context(TokenError::Invalid),
                300 => err.context(FingerprintError),
                _ => err,
            });
        }

        Err(anyhow!(gettext!(
            "Got invalid status response of {}",
            self.status
        )))
    }
}
