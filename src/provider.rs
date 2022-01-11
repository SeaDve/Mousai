use std::path::Path;

use downcast_rs::{impl_downcast, Downcast};

use crate::model::Song;

pub trait Provider: Downcast {
    // TODO consider inputting bytes directly
    fn recognize(&self, path: &Path) -> anyhow::Result<Song>;
}

impl_downcast!(Provider);

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn downcast() {
        struct ProviderOne;

        impl Provider for ProviderOne {
            fn recognize(&self, _: &Path) -> anyhow::Result<Song> {
                Ok(Song::new("", "", ""))
            }
        }

        struct ProviderTwo;

        impl Provider for ProviderTwo {
            fn recognize(&self, _: &Path) -> anyhow::Result<Song> {
                Ok(Song::new("", "", ""))
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
