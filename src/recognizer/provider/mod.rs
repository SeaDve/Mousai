mod aud_d;

use async_trait::async_trait;
use gettextrs::gettext;
use gtk::glib;
use once_cell::sync::OnceCell;

use std::{
    sync::{Mutex, MutexGuard},
    time::Duration,
};

use self::aud_d::{AudD, AudDMock};
use crate::{core::AudioRecording, model::Song};

#[async_trait(?Send)]
pub trait Provider: std::fmt::Debug {
    /// Recognize a song from a recording
    async fn recognize(&self, recording: &AudioRecording) -> Result<Song, ProviderError>;

    /// How long to record the audio
    fn listen_duration(&self) -> Duration;

    /// Whether this supports TestProviderMode
    fn is_test(&self) -> bool;
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, glib::Enum)]
#[enum_type(name = "MsaiTestProviderMode")]
pub enum TestProviderMode {
    ErrorOnly,
    #[default]
    ValidOnly,
    Both,
}

impl From<i32> for TestProviderMode {
    fn from(val: i32) -> Self {
        use glib::translate::TryFromGlib;
        unsafe { Self::try_from_glib(val) }
            .unwrap_or_else(|err| panic!("Failed to turn `{val}` into TestProviderMode: {err:?}"))
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, glib::Enum)]
#[enum_type(name = "MsaiProviderType")]
pub enum ProviderType {
    #[default]
    AudD,
    AudDMock,
}

impl ProviderType {
    pub fn to_provider(self) -> Box<dyn Provider> {
        match self {
            Self::AudD => Box::new(AudD::default()),
            Self::AudDMock => Box::new(AudDMock),
        }
    }
}

impl From<i32> for ProviderType {
    fn from(val: i32) -> Self {
        use glib::translate::TryFromGlib;
        unsafe { Self::try_from_glib(val) }
            .unwrap_or_else(|err| panic!("Failed to turn `{val}` into ProviderType: {err:?}"))
    }
}

#[derive(Debug)]
pub struct ProviderSettings {
    pub active: ProviderType,
    pub test_mode: TestProviderMode,
    pub test_listen_duration: Duration,
    pub test_recognize_duration: Duration,
}

impl ProviderSettings {
    /// Acquire lock to the global `ProviderSettings`
    pub fn lock() -> MutexGuard<'static, Self> {
        static INSTANCE: OnceCell<Mutex<ProviderSettings>> = OnceCell::new();

        INSTANCE
            .get_or_init(|| Mutex::new(ProviderSettings::default()))
            .lock()
            .expect("Failed to lock the global ProviderSettings mutex")
    }

    /// Reset all fields to their defaults
    pub fn reset(&mut self) {
        *self = Self::default();
    }
}

impl Default for ProviderSettings {
    fn default() -> Self {
        Self {
            active: ProviderType::default(),
            test_mode: TestProviderMode::default(),
            test_listen_duration: Duration::from_secs(1),
            test_recognize_duration: Duration::from_secs(1),
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum ProviderError {
    NoMatches,
    NoToken(String),
    InvalidToken,
    Connection(String),
    Other(String),
}

impl std::fmt::Display for ProviderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProviderError::Connection(string) => {
                f.write_str(&gettext!("{} Check your internet connection.", string))
            }
            ProviderError::NoMatches => f.write_str(&gettext("No matches found for this song.")),
            ProviderError::NoToken(string) => f.write_str(&gettext!(
                "{} Input an API token in the preferences.",
                string
            )),
            ProviderError::InvalidToken => f.write_str(&gettext("Please input a valid API token.")),
            ProviderError::Other(string) => f.write_str(string),
        }
    }
}

impl std::error::Error for ProviderError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[gtk::test] // Run in serial
    fn provider_settings_reset() {
        let mut settings = ProviderSettings::lock();
        assert_eq!(settings.active, ProviderType::default());

        settings.active = ProviderType::AudDMock;
        assert_ne!(settings.active, ProviderType::default());

        settings.reset();
        assert_eq!(settings.active, ProviderType::default());
    }

    #[gtk::test] // Run in serial
    fn provider_settings_identity() {
        let mut lock_a = ProviderSettings::lock();
        lock_a.active = ProviderType::AudDMock;
        let active = lock_a.active;
        drop(lock_a);

        let mut lock_b = ProviderSettings::lock();
        assert_eq!(lock_b.active, active);

        lock_b.reset();
        assert_ne!(lock_b.active, active);
    }
}
