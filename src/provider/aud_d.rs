use async_trait::async_trait;
use reqwest::Client;
use serde::Deserialize;
use serde_json::json;

use std::time::Duration;

use super::Provider;
use crate::{core::AudioRecording, model::Song, utils, RUNTIME};

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
}

#[derive(Debug, Deserialize)]
pub struct Data {
    pub title: String,
    pub artist: String,
    pub timecode: String,
    #[serde(rename(deserialize = "song_link"))]
    pub info_link: String,
    #[serde(rename(deserialize = "spotify"))]
    pub spotify_data: SpotifyData,
}

#[derive(Debug, Deserialize)]
pub struct Response {
    #[serde(rename(deserialize = "result"))]
    pub data: Option<Data>,
    pub status: String,
}

#[derive(Debug)]
pub struct AudD {
    api_token: String,
}

impl AudD {
    pub fn new(api_token: Option<&str>) -> Self {
        Self {
            api_token: api_token.unwrap_or_default().to_string(),
        }
    }
}

#[async_trait(?Send)]
impl Provider for AudD {
    async fn recognize(&self, recording: &AudioRecording) -> anyhow::Result<Song> {
        let data = json!({
            "api_token": self.api_token,
            "return": "spotify",
            "audio": utils::file_to_base64(recording.path()).await?,
        });

        let response = RUNTIME
            .spawn(async move {
                Client::new()
                    .post("https://api.audd.io/")
                    .body(data.to_string())
                    .send()
                    .await
            })
            .await
            .unwrap()?;

        let response: Response = RUNTIME
            .spawn(async move {
                let full = response.bytes().await.unwrap();
                dbg!(&full);
                serde_json::from_slice(&full)
            })
            .await
            .unwrap()?;

        anyhow::ensure!(
            response.status == "success",
            "Returned {} as status",
            response.status
        );

        let data = response
            .data
            .ok_or(anyhow::anyhow!("Cannot recognize song"))?;

        Ok(Song::new(&data.title, &data.artist, &data.info_link))
    }

    fn listen_duration(&self) -> Duration {
        Duration::from_secs(5)
    }
}

impl Default for AudD {
    fn default() -> Self {
        Self::new(None)
    }
}
