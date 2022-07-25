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

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, glib::Enum)]
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

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, glib::Enum)]
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

#[derive(Debug, PartialEq, Eq)]
pub struct ProviderManager {
    pub active: ProviderType,
    pub test_mode: TestProviderMode,
    pub test_listen_duration: Duration,
    pub test_recognize_duration: Duration,
}

impl ProviderManager {
    /// Acquire lock to the global `ProviderManager`
    pub fn lock() -> MutexGuard<'static, Self> {
        static PROVIDER_MANAGER: OnceCell<Mutex<ProviderManager>> = OnceCell::new();

        PROVIDER_MANAGER
            .get_or_init(|| Mutex::new(ProviderManager::default()))
            .lock()
            .unwrap()
    }

    /// Reset all fields to their defaults
    pub fn reset(&mut self) {
        *self = Self::default();
    }
}

impl Default for ProviderManager {
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

    #[test]
    fn reset_provider_manager() {
        let mut manager = ProviderManager::lock();
        assert_eq!(*manager, ProviderManager::default());

        manager.active = ProviderType::AudDMock;
        assert_ne!(*manager, ProviderManager::default());

        manager.reset();
        assert_eq!(*manager, ProviderManager::default());
    }

    #[test]
    fn provider_manager_identity() {
        let mut lock = ProviderManager::lock();
        lock.active = ProviderType::AudDMock;
        let active = lock.active;
        drop(lock);

        assert_eq!(ProviderManager::lock().active, active);
    }
}
