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
            {
                let song = Song::new("Amnesia", "5 Seconds Of Summer", "https://lis.tn/WSKAzD");
                song.set_album_art_link(Some(
                    "https://i.scdn.co/image/ab67616d0000b27393432e914046a003229378da",
                ));
                song
            },
            {
                let song = Song::new(
                    "Scars To Your Beautiful",
                    "Alessia Cara",
                    "https://lis.tn/ScarsToYourBeautiful",
                );
                song.set_album_art_link(Some(
                    "https://i.scdn.co/image/ab67616d0000b273e3ae597159d6c2541c4ee61b",
                ));
                song
            },
            {
                let song = Song::new("Beautiful Sunday", "Daniel Boone", "https://lis.tn/YTuccJ");
                song.set_album_art_link(Some(
                    "https://i.scdn.co/image/ab67616d0000b273db8f64a52a4ec4cde9a9528a",
                ));
                song
            },
            {
                let song = Song::new(
                    "The Night We Met",
                    "Lord Huron",
                    "https://lis.tn/TheNightWeMet",
                );
                song.set_album_art_link(Some("https://some.invalid.link"));
                song
            },
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
