mod aud_d;
mod error;
mod settings;

use anyhow::Result;
use async_trait::async_trait;

use std::{fmt, time::Duration};

pub use self::settings::{ProviderSettings, ProviderType, TestProviderMode};
use crate::{audio_recording::AudioRecording, model::Song};

#[async_trait(?Send)]
pub trait Provider: fmt::Debug {
    /// Recognize a song from a recording
    async fn recognize(&self, recording: &AudioRecording) -> Result<Song>;

    /// How long to record the audio
    fn listen_duration(&self) -> Duration;

    /// Whether this supports `TestProviderMode`
    fn is_test(&self) -> bool {
        false
    }
}

#[async_trait(?Send)]
pub trait TestProvider {
    async fn recognize_impl(
        &self,
        recording: &AudioRecording,
        mode: TestProviderMode,
    ) -> Result<Song>;
}

#[async_trait(?Send)]
impl<T> Provider for T
where
    T: TestProvider + fmt::Debug,
{
    async fn recognize(&self, recording: &AudioRecording) -> Result<Song> {
        let duration = ProviderSettings::lock().test_recognize_duration;
        glib::timeout_future(duration).await;

        let mode = ProviderSettings::lock().test_mode;
        self.recognize_impl(recording, mode).await
    }

    fn listen_duration(&self) -> Duration {
        ProviderSettings::lock().test_listen_duration
    }

    fn is_test(&self) -> bool {
        true
    }
}
