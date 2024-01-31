use anyhow::{anyhow, Error, Result};
use gtk::glib::{self, translate::TryFromGlib};

use std::{
    sync::{Mutex, MutexGuard, OnceLock},
    time::Duration,
};

use super::Provider;
use crate::Application;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, glib::Enum)]
#[enum_type(name = "MsaiTestProviderMode")]
pub enum TestProviderMode {
    ErrorOnly,
    #[default]
    ValidOnly,
    Both,
}

impl TryFrom<i32> for TestProviderMode {
    type Error = Error;

    fn try_from(val: i32) -> Result<Self> {
        unsafe { Self::try_from_glib(val) }.map_err(|_| anyhow!("Invalid value `{}`", val))
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, glib::Enum)]
#[enum_type(name = "MsaiProviderType")]
pub enum ProviderType {
    #[default]
    AudD,
    AudDMock,
    ErrorTester,
}

impl ProviderType {
    pub fn to_provider(self) -> Box<dyn Provider> {
        use super::{
            aud_d::{AudD, AudDMock},
            error_tester::ErrorTester,
        };

        match self {
            Self::AudD => {
                // FIXME handle this outside
                let api_token = Application::get().settings().aud_d_api_token();
                Box::new(AudD::new(Some(&api_token)))
            }
            Self::AudDMock => Box::new(AudDMock),
            Self::ErrorTester => Box::new(ErrorTester),
        }
    }
}

impl TryFrom<i32> for ProviderType {
    type Error = Error;

    fn try_from(val: i32) -> Result<Self> {
        unsafe { Self::try_from_glib(val) }.map_err(|_| anyhow!("Invalid value `{}`", val))
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
        static INSTANCE: OnceLock<Mutex<ProviderSettings>> = OnceLock::new();

        INSTANCE
            .get_or_init(|| Mutex::new(ProviderSettings::default()))
            .lock()
            .unwrap()
    }

    /// Reset all fields to their defaults
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    fn default() -> Self {
        Self {
            active: ProviderType::default(),
            test_mode: TestProviderMode::default(),
            test_listen_duration: Duration::from_secs(1),
            test_recognize_duration: Duration::from_secs(1),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[gtk::test] // Run in serial
    fn reset() {
        let mut settings = ProviderSettings::lock();
        assert_eq!(settings.active, ProviderType::default());

        settings.active = ProviderType::AudDMock;
        assert_ne!(settings.active, ProviderType::default());

        settings.reset();
        assert_eq!(settings.active, ProviderType::default());
    }

    #[gtk::test] // Run in serial
    fn identity() {
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
