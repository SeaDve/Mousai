mod aud_d;
mod mock;

pub use self::{aud_d::AudD, mock::Mock};

use async_trait::async_trait;

use std::time::Duration;

use crate::{core::AudioRecording, model::Song};

// TODO: more generic error, so it won't have to be an enum for each provider
#[derive(Debug, thiserror::Error)]
pub enum ProviderError {
    #[error("AudD Provider Error: {0}")]
    AudD(aud_d::Error),
    #[error("Other Provider Error: {0}")]
    Other(String),
}

#[async_trait(?Send)]
pub trait Provider: std::fmt::Debug {
    async fn recognize(&self, recording: &AudioRecording) -> Result<Song, ProviderError>;

    fn listen_duration(&self) -> Duration;
}
