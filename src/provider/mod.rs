mod aud_d;
mod mock;

pub use self::{aud_d::AudD, mock::Mock};

use async_trait::async_trait;

use std::time::Duration;

use crate::{core::AudioRecording, model::Song};

#[async_trait(?Send)]
pub trait Provider: std::fmt::Debug {
    async fn recognize(&self, recording: &AudioRecording) -> anyhow::Result<Song>;

    fn listen_duration(&self) -> Duration;
}
