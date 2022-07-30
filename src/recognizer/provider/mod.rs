mod aud_d;
mod error;
mod settings;

use async_trait::async_trait;

use std::time::Duration;

use self::error::ProviderError;
pub use self::settings::{ProviderSettings, ProviderType, TestProviderMode};
use crate::{core::AudioRecording, model::Song};

#[async_trait(?Send)]
pub trait Provider: std::fmt::Debug {
    /// Recognize a song from a recording
    async fn recognize(&self, recording: &AudioRecording) -> Result<Song, ProviderError>;

    /// How long to record the audio
    fn listen_duration(&self) -> Duration;

    /// Whether this supports `TestProviderMode`
    fn is_test(&self) -> bool;
}
