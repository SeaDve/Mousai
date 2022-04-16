use gtk::{
    gdk,
    gio::{self, prelude::*},
};

use std::{
    path::PathBuf,
    rc::{Rc, Weak},
};

use super::AlbumArtStoreInner;

#[derive(Debug, Clone)]
pub struct AlbumArt {
    download_url: String,
    cache_path: PathBuf,
    store: Weak<AlbumArtStoreInner>,
}

impl AlbumArt {
    pub(super) fn new(download_url: &str, store: &Rc<AlbumArtStoreInner>) -> Self {
        Self {
            download_url: download_url.to_string(),
            cache_path: store
                .cache_dir()
                .join(download_url.to_string().replace('/', "-")),
            store: Rc::downgrade(store),
        }
    }

    pub fn uri(&self) -> String {
        let cache_file = gio::File::for_path(self.cache_path.as_path());

        if cache_file.query_exists(gio::Cancellable::NONE) {
            return cache_file.uri().into();
        }

        self.download_url.clone()
    }

    pub async fn texture(&self) -> anyhow::Result<gdk::Texture> {
        self.store
            .upgrade()
            .ok_or_else(|| anyhow::anyhow!("Failed to upgrade store."))?
            .get_or_try_load_texture(&self.download_url, &self.cache_path)
            .await
    }
}
