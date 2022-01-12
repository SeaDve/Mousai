mod aud_d;
mod mock;

pub use self::{aud_d::AudD, mock::Mock};

use async_trait::async_trait;
use downcast_rs::{impl_downcast, Downcast};

use std::time::Duration;

use crate::{core::AudioRecording, model::Song};

#[async_trait(?Send)]
pub trait Provider: Downcast + std::fmt::Debug {
    async fn recognize(&self, recording: &AudioRecording) -> anyhow::Result<Song>;

    fn listen_duration(&self) -> Duration;
}

impl_downcast!(Provider);

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn downcast() {
        #[derive(Debug)]
        struct ProviderOne;

        #[async_trait(?Send)]
        impl Provider for ProviderOne {
            async fn recognize(&self, _: &AudioRecording) -> anyhow::Result<Song> {
                Ok(Song::new("", "", ""))
            }

            fn listen_duration(&self) -> Duration {
                Duration::ZERO
            }
        }

        #[derive(Debug)]
        struct ProviderTwo;

        #[async_trait(?Send)]
        impl Provider for ProviderTwo {
            async fn recognize(&self, _: &AudioRecording) -> anyhow::Result<Song> {
                Ok(Song::new("", "", ""))
            }

            fn listen_duration(&self) -> Duration {
                Duration::ZERO
            }
        }

        let provider_one: Box<dyn Provider> = Box::new(ProviderOne);
        assert!(provider_one.downcast_ref::<ProviderOne>().is_some());
        assert!(provider_one.downcast_ref::<ProviderTwo>().is_none());

        let provider_two: Box<dyn Provider> = Box::new(ProviderTwo);
        assert!(provider_two.downcast_ref::<ProviderOne>().is_none());
        assert!(provider_two.downcast_ref::<ProviderTwo>().is_some());
    }
}
