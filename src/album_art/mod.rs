mod store;

use gtk::{
    gdk,
    gio::{self, prelude::*},
    glib,
};
use once_cell::sync::Lazy;

use std::{fs, path::PathBuf};

use self::store::Store;
use crate::model::{Song, SongId};

static CACHE: Lazy<Store> = Lazy::new(Store::default);

/// Cache a texture into a static store for song
#[derive(Debug, Clone)]
pub struct AlbumArt {
    pub(self) cache_path: PathBuf,
    pub(self) download_url: String,
    pub(self) song_id: SongId,
}

impl AlbumArt {
    pub fn init_cache_dir() -> anyhow::Result<()> {
        let cache_dir = default_album_art_cache_dir();
        Ok(fs::create_dir_all(cache_dir)?)
    }

    pub fn for_song(song: &Song) -> anyhow::Result<Self> {
        let cache_path = {
            let mut path = default_album_art_cache_dir();
            path.push(song.id().to_string().replace('/', "-"));
            path
        };

        let download_url = song
            .album_art_link()
            .ok_or_else(|| anyhow::anyhow!("Song doesn't have album art link"))?;

        Ok(Self {
            cache_path,
            download_url,
            song_id: song.id(),
        })
    }

    pub fn uri(&self) -> String {
        let cache_file = gio::File::for_path(self.cache_path.as_path());

        if cache_file.query_exists(gio::Cancellable::NONE) {
            return cache_file.uri().into();
        }

        self.download_url.clone()
    }

    pub async fn texture(&self) -> anyhow::Result<gdk::Texture> {
        CACHE.get_or_try_load(self).await
    }
}

fn default_album_art_cache_dir() -> PathBuf {
    let mut path = glib::user_cache_dir();
    path.push("album_art_cache");
    path
}
