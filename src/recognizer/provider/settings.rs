use gtk::glib;
use once_cell::sync::OnceCell;

use std::{
    sync::{Mutex, MutexGuard},
    time::Duration,
};

use super::Provider;

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
        use super::aud_d::{AudD, AudDMock};

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
