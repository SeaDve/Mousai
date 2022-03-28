use async_trait::async_trait;
use gtk::glib;
use rand::seq::SliceRandom;

use std::time::Duration;

use super::{Provider, ProviderError};
use crate::{core::AudioRecording, model::Song};

#[derive(Debug)]
pub struct Mock;

#[async_trait(?Send)]
impl Provider for Mock {
    async fn recognize(&self, _: &AudioRecording) -> Result<Song, ProviderError> {
        let rand_song = [
            Song::new(
                "Eine Kleine Nachtmusik",
                "The London Symphony Orchestra",
                "https://lis.tn/EineKleineNachtmusik",
            ),
            Song::new("Make You Mine", "Public", "https://lis.tn/FUYgUV"),
            Song::new("Amnesia", "5 Seconds Of Summer", "https://lis.tn/WSKAzD"),
            Song::new(
                "Scars To Your Beautiful",
                "Alessia Cara",
                "https://lis.tn/ScarsToYourBeautiful",
            ),
            Song::new("Beautiful Sunday", "Daniel Boone", "https://lis.tn/YTuccJ"),
            Song::new(
                "The Night We Met",
                "Lord Huron",
                "https://lis.tn/TheNightWeMet",
            ),
        ]
        .choose(&mut rand::thread_rng())
        .cloned()
        .ok_or_else(|| ProviderError::Other("Failed to generate random song".into()))?;

        glib::timeout_future(Duration::from_secs(1)).await;

        log::info!(
            "Recognized Song: {} - {}",
            rand_song.artist(),
            rand_song.title()
        );

        Ok(rand_song)
    }

    fn listen_duration(&self) -> Duration {
        Duration::from_secs(1)
    }
}
