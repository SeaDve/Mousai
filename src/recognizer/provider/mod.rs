mod aud_d;

use async_trait::async_trait;
use gettextrs::gettext;
use gtk::glib;
use once_cell::sync::OnceCell;

use std::{sync::RwLock, time::Duration};

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

#[derive(Default, Debug, Clone, Copy, glib::Enum)]
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

#[derive(Default, Debug, Clone, Copy, glib::Enum)]
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
pub struct ProviderManager {
    active: RwLock<ProviderType>,
    test_mode: RwLock<TestProviderMode>,
    test_listen_duration: RwLock<Duration>,
    test_recognize_duration: RwLock<Duration>,
}

impl ProviderManager {
    pub fn global() -> &'static Self {
        static PROVIDER_MANAGER: OnceCell<ProviderManager> = OnceCell::new();

        PROVIDER_MANAGER.get_or_init(|| {
            let this = Self {
                active: Default::default(),
                test_mode: Default::default(),
                test_listen_duration: Default::default(),
                test_recognize_duration: Default::default(),
            };
            this.reset_test_durations();
            this
        })
    }

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
