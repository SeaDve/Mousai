use async_trait::async_trait;

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use super::Provider;
use crate::{core::AudioRecording, model::Song};

#[derive(Debug)]
pub struct Mock;

#[async_trait(?Send)]
impl Provider for Mock {
    async fn recognize(&self, _: &AudioRecording) -> anyhow::Result<Song> {
        let now = SystemTime::now();

        let since_the_epoch = now.duration_since(UNIX_EPOCH)?;

        Ok(Song::new(
            "A song",
            "Sang by me",
            &since_the_epoch.as_secs().to_string(),
        ))
    }

    fn listen_duration(&self) -> Duration {
        Duration::from_secs(2)
    }
}
