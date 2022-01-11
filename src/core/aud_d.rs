use reqwest::Client;
use serde::Deserialize;
use serde_json::json;

use std::path::Path;

use crate::utils;

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
    pub result: Data,
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

    pub async fn recognize(&self, path: impl AsRef<Path>) -> anyhow::Result<Response> {
        let data = json!({
            "api_token": self.api_token,
            "return": "spotify",
            "audio": utils::file_to_base64(path).await?,
        });

        let client = Client::new();
        let response = client
            .post("https://api.audd.io/")
            .body(data.to_string())
            .send()
            .await?;

        let response: Response = response.json().await?;

        anyhow::ensure!(response.status == "success", "Unable to recognize song");

        Ok(response)
    }
}
