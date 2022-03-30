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
            {
                let song = Song::new("Make You Mine", "Public", "https://lis.tn/FUYgUV");
                song.set_playback_link(Some("https://listen.hs.llnwd.net/g3/prvw/1/3/1/4/2/1119824131.mp3"));
                song
            },
            {
                let song = Song::new("Amnesia", "5 Seconds Of Summer", "https://lis.tn/WSKAzD");
                song.set_album_art_link(Some(
                    "https://i.scdn.co/image/ab67616d0000b27393432e914046a003229378da",
                ));
                song.set_playback_link(Some(
                    "https://audio-ssl.itunes.apple.com/itunes-assets/AudioPreview125/v4/9d/bf/3f/9dbf3f13-ae71-816f-44c1-9a6c4358e0b2/mzaf_10286406549181982375.plus.aac.p.m4a",
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
                song.set_playback_link(Some("https://audio-ssl.itunes.apple.com/itunes-assets/AudioPreview126/v4/22/67/5a/22675a39-eaa1-1059-f59f-9663fbbb513a/mzaf_9692204965232946435.plus.aac.p.m4a"));
                song
            },
            {
                let song = Song::new("Beautiful Sunday", "Daniel Boone", "https://lis.tn/YTuccJ");
                song.set_album_art_link(Some(
                    "https://i.scdn.co/image/ab67616d0000b273db8f64a52a4ec4cde9a9528a",
                ));
                song.set_playback_link(Some("https://p.scdn.co/mp3-preview/b2fa24732fe08a251b0c8d44774f37fd55378378?cid=e44e7b8278114c7db211c00ea273ac69"));
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
