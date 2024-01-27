mod aud_d;
mod error;
mod error_tester;
mod settings;

use async_trait::async_trait;
use gtk::glib;

use std::{fmt, time::Duration};

pub use self::{
    error::{RecognizeError, RecognizeErrorKind},
    settings::{ProviderSettings, ProviderType, TestProviderMode},
};
use crate::song::Song;

#[async_trait(?Send)]
pub trait Provider: fmt::Debug {
    /// Recognize a song from bytes
    async fn recognize(&self, bytes: &[u8]) -> Result<Song, RecognizeError>;

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
        bytes: &[u8],
        mode: TestProviderMode,
    ) -> Result<Song, RecognizeError>;
}

#[async_trait(?Send)]
impl<T> Provider for T
where
    T: TestProvider + fmt::Debug,
{
    async fn recognize(&self, bytes: &[u8]) -> Result<Song, RecognizeError> {
        let duration = ProviderSettings::lock().test_recognize_duration;
        glib::timeout_future(duration).await;

        let mode = ProviderSettings::lock().test_mode;
        self.recognize_impl(bytes, mode).await
    }

    fn listen_duration(&self) -> Duration {
        ProviderSettings::lock().test_listen_duration
    }

    fn is_test(&self) -> bool {
        true
    }
}
