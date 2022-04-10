mod aud_d;

use async_trait::async_trait;
use gettextrs::gettext;
use gtk::glib;
use once_cell::sync::Lazy;

use std::{sync::RwLock, time::Duration};

use self::aud_d::{AudD, AudDMock};
use crate::{core::AudioRecording, model::Song};

pub static PROVIDER_MANAGER: Lazy<ProviderManager> = Lazy::new(ProviderManager::default);

#[async_trait(?Send)]
pub trait Provider: std::fmt::Debug {
    /// Recognize a song from a recording
    async fn recognize(&self, recording: &AudioRecording) -> Result<Song, ProviderError>;

    /// How long to record the audio
    fn listen_duration(&self) -> Duration;

    /// Whether this supports TestProviderMode
    fn is_test(&self) -> bool;
}

#[derive(Debug, Clone, Copy, glib::Enum)]
#[enum_type(name = "MsaiTestProviderMode")]
pub enum TestProviderMode {
    ErrorOnly,
    ValidOnly,
    Both,
}

impl Default for TestProviderMode {
    fn default() -> Self {
        Self::ValidOnly
    }
}

impl From<i32> for TestProviderMode {
    fn from(val: i32) -> Self {
        use glib::translate::TryFromGlib;
        unsafe { Self::try_from_glib(val) }
            .unwrap_or_else(|err| panic!("Failed to turn `{val}` into TestProviderMode: {err:?}"))
    }
}

#[derive(Debug, Clone, Copy, glib::Enum)]
#[enum_type(name = "MsaiProviderType")]
pub enum ProviderType {
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

impl Default for ProviderType {
    fn default() -> Self {
        Self::AudD
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
pub struct ProviderManager {
    active: RwLock<ProviderType>,
    test_mode: RwLock<TestProviderMode>,
    test_listen_duration: RwLock<Duration>,
    test_recognize_duration: RwLock<Duration>,
}

impl Default for ProviderManager {
    fn default() -> Self {
        let obj = Self {
            active: Default::default(),
            test_mode: Default::default(),
            test_listen_duration: Default::default(),
            test_recognize_duration: Default::default(),
        };
        obj.reset_test_durations();
        obj
    }
}

impl ProviderManager {
    pub fn active(&self) -> ProviderType {
        *self.active.read().unwrap()
    }

    pub fn set_active(&self, new_value: ProviderType) {
        *self.active.write().unwrap() = new_value;
    }

    pub fn test_mode(&self) -> TestProviderMode {
        *self.test_mode.read().unwrap()
    }

    pub fn set_test_mode(&self, new_value: TestProviderMode) {
        *self.test_mode.write().unwrap() = new_value;
    }

    pub fn test_listen_duration(&self) -> Duration {
        *self.test_listen_duration.read().unwrap()
    }

    pub fn set_test_listen_duration(&self, new_value: Duration) {
        *self.test_listen_duration.write().unwrap() = new_value;
    }

    pub fn test_recognize_duration(&self) -> Duration {
        *self.test_recognize_duration.read().unwrap()
    }

    pub fn set_test_recognize_duration(&self, new_value: Duration) {
        *self.test_recognize_duration.write().unwrap() = new_value;
    }

    pub fn reset_active(&self) {
        self.set_active(ProviderType::default());
    }

    pub fn reset_test_mode(&self) {
        self.set_test_mode(TestProviderMode::default());
    }

    pub fn reset_test_durations(&self) {
        self.set_test_listen_duration(Duration::from_secs(1));
        self.set_test_recognize_duration(Duration::from_secs(1));
    }
}

#[derive(Debug)]
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
            ProviderError::Connection(string) => f.write_str(string),
            ProviderError::NoMatches => f.write_str(&gettext("No matches found for this song.")),
            ProviderError::NoToken(string) => {
                f.write_str(&gettext!("{} Please input an API token.", string))
            }
            ProviderError::InvalidToken => f.write_str(&gettext("Please input a valid API token.")),
            ProviderError::Other(string) => f.write_str(string),
        }
    }
}

impl std::error::Error for ProviderError {}
