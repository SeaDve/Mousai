mod aud_d;
mod mock;

pub use self::{aud_d::AudD, mock::Mock};

use async_trait::async_trait;

use std::time::Duration;

use crate::{core::AudioRecording, model::Song};

// TODO: more generic error, so it won't have to be an enum for each provider
#[derive(Debug)]
pub enum ProviderError {
    AudD(aud_d::Error),
}

impl std::fmt::Display for ProviderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::AudD(err) => std::fmt::Display::fmt(err, f),
        }
    }
}

impl std::error::Error for ProviderError {}

#[async_trait(?Send)]
pub trait Provider: std::fmt::Debug {
    async fn recognize(&self, recording: &AudioRecording) -> Result<Song, ProviderError>;

    fn listen_duration(&self) -> Duration;
}
