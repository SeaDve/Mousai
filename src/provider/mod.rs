mod aud_d;
mod mock;

use async_trait::async_trait;
use gtk::glib;
use once_cell::sync::Lazy;

use std::{sync::RwLock, time::Duration};

use self::{aud_d::AudD, mock::Mock};
use crate::{core::AudioRecording, model::Song};

pub static PROVIDER_MANAGER: Lazy<ProviderManager> = Lazy::new(ProviderManager::default);

#[derive(Debug, Clone, Copy, glib::Enum)]
#[enum_type(name = "MsaiProviderType")]
pub enum ProviderType {
    AudD,
    Mock,
}

impl ProviderType {
    pub fn to_provider(self) -> Box<dyn Provider> {
        match self {
            Self::AudD => Box::new(AudD::default()),
            Self::Mock => Box::new(Mock),
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
        unsafe { Self::try_from_glib(dbg!(val)) }
            .unwrap_or_else(|err| panic!("Failed to turn `{val}` into ProviderType: {err:?}"))
    }
}

#[derive(Debug, Default)]
pub struct ProviderManager {
    active: RwLock<ProviderType>,
}

impl ProviderManager {
    pub fn active(&self) -> ProviderType {
        *self.active.read().unwrap()
    }

    pub fn set_active(&self, new_value: ProviderType) {
        let mut active = self.active.write().unwrap();
        *active = new_value;
    }

    pub fn reset_active(&self) {
        self.set_active(ProviderType::default());
    }
}

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
