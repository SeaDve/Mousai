use async_trait::async_trait;
use gtk::glib;
use rand::Rng;

use std::time::Duration;

use super::Provider;
use crate::{core::AudioRecording, model::Song};

#[derive(Debug)]
pub struct Mock;

#[async_trait(?Send)]
impl Provider for Mock {
    async fn recognize(&self, _: &AudioRecording) -> anyhow::Result<Song> {
        let rand_title: String = rand::thread_rng()
            .sample_iter(&rand::distributions::Alphanumeric)
            .take(30)
            .map(char::from)
            .collect();

        let rand_artist: String = rand::thread_rng()
            .sample_iter(&rand::distributions::Alphanumeric)
            .take(30)
            .map(char::from)
            .collect();

        let rand_link: String = rand::thread_rng()
            .sample_iter(&rand::distributions::Alphanumeric)
            .take(30)
            .map(char::from)
            .collect();

        glib::timeout_future(Duration::from_secs(1)).await;

        Ok(Song::new(&rand_title, &rand_artist, &rand_link))
    }

    fn listen_duration(&self) -> Duration {
        Duration::from_secs(3)
    }
}
