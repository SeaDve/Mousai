use gtk::{gdk, glib, prelude::*};
use reqwest::Client;

use std::{collections::HashMap, sync::Mutex};

use crate::{
    model::{Song, SongId},
    RUNTIME,
};

pub struct AlbumArtManager {
    store: Mutex<HashMap<SongId, gdk::Texture>>,
    client: reqwest::Client,
}

impl AlbumArtManager {
    pub fn new() -> Self {
        AlbumArtManager {
            store: Mutex::new(HashMap::new()),
            client: Client::new(),
        }
    }

    pub async fn get_or_init(&self, song: &Song) -> anyhow::Result<gdk::Texture> {
        {
            let store = self
                .store
                .lock()
                .expect("Failed to lock album art store mutex");
            if let Some(texture) = store.get(&song.id()) {
                return Ok(texture.clone());
            }
        }

        // FIXME more reliable way to create unique cache path
        let cache_path = glib::user_cache_dir().join(song.id().try_to_string()?);

        if let Ok(texture) = gdk::Texture::from_filename(&cache_path) {
            let mut store = self
                .store
                .lock()
                .expect("Failed to lock album art store mutex");
            store.insert(song.id(), texture.clone());

            return Ok(texture);
        }

        if let Some(album_art_link) = song.album_art_link() {
            let response = RUNTIME
                .spawn(self.client.get(&album_art_link).send())
                .await??;
            let bytes = RUNTIME.spawn(response.bytes()).await??;
            log::info!("Downloaded album art link from `{album_art_link}`");

            let texture = gdk::Texture::from_bytes(&glib::Bytes::from_owned(bytes))?;
            texture.save_to_png(cache_path)?;

            {
                let mut store = self
                    .store
                    .lock()
                    .expect("Failed to lock album art store mutex");
                store.insert(song.id(), texture.clone());
            }

            return Ok(texture);
        }

        Err(anyhow::anyhow!(
            "Song doesn't have a provided `album_art_link`"
        ))
    }
}
