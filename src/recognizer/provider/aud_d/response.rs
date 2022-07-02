use gettextrs::gettext;
use serde::Deserialize;

use super::ProviderError;

#[derive(Debug, Deserialize)]
pub struct LyricsData {
    pub lyrics: String,
    /// A json string containing `provider` and `url` field
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
    pub label: String,
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
    pub fn parse(slice: &[u8]) -> Result<Self, ProviderError> {
        serde_json::from_slice(slice).map_err(|err| {
            log::error!("Failed to parse response: {:?}", err);

            ProviderError::Other(gettext(
                "Failed to parse response. Please report this to Mousai's bug tracker",
            ))
        })
    }

    pub fn data(self) -> Result<Data, ProviderError> {
        match self.status.as_str() {
            "success" => self.data.map_or_else(|| Err(ProviderError::NoMatches), Ok),
            "error" => Err(self.error.map_or_else(
                || ProviderError::Other("Got `error` status, but no error".into()),
                |error| {
                    // Based on https://docs.audd.io/#common-errors
                    match error.code {
                        901 => ProviderError::NoToken(gettext("Daily limit has been reached.")),
                        900 => ProviderError::InvalidToken,
                        300 => ProviderError::Other(gettext(
                            "Failed to fingerprint audio. There may be no sound heard.",
                        )),
                        other_code => {
                            ProviderError::Other(format!("{} ({})", error.message, other_code))
                        }
                    }
                },
            )),
            other => Err(ProviderError::Other(gettext!(
                "Got invalid status response of {}",
                other
            ))),
        }
    }
}
