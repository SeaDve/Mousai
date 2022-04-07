use gtk::{
    gdk::{self, prelude::*},
    gio, glib,
};
use reqwest::Client;

use std::{collections::HashMap, sync::RwLock};

use super::AlbumArt;
use crate::{model::SongId, RUNTIME};

pub struct Store {
    store: RwLock<HashMap<SongId, gdk::Texture>>,
    client: reqwest::Client,
}

impl Store {
    pub fn new() -> Self {
        Self {
            store: RwLock::new(HashMap::new()),
            client: Client::new(),
        }
    }

    pub async fn get_or_try_load(&self, album_art: &AlbumArt) -> anyhow::Result<gdk::Texture> {
        if let Some(texture) = self.get(&album_art.song_id) {
            return Ok(texture.clone());
        }

        let cache_path = &album_art.cache_path;

        match gio::File::for_path(cache_path).load_bytes_future().await {
            Ok((ref bytes, _)) => {
                let texture = gdk::Texture::from_bytes(bytes)?;
                self.insert(album_art.song_id.clone(), texture.clone());
                return Ok(texture);
            }
            Err(err) => log::warn!(
                "Failed to load file from path `{}`: {:?}",
                cache_path.display(),
                err
            ),
        }

        // FIXME thumbnail can be downloaded twice when it is not in disk and tried to
        // call this again before it has finished downloading.
        let download_url = &album_art.download_url;

        let response = RUNTIME
            .spawn(self.client.get(download_url).send())
            .await??;
        let bytes = RUNTIME.spawn(response.bytes()).await??;
        log::info!("Downloaded album art from link `{download_url}`");

        let texture = gdk::Texture::from_bytes(&glib::Bytes::from_owned(bytes))?;
        texture.save_to_png(cache_path)?;

        self.insert(album_art.song_id.clone(), texture.clone());

        return Ok(texture);
    }

    fn get(&self, id: &SongId) -> Option<gdk::Texture> {
        match self.store.read() {
            Ok(store) => store.get(id).cloned(),
            Err(err) => {
                log::warn!("Failed to read from store: {err:?}");
                None
            }
        }
    }

    fn insert(&self, id: SongId, texture: gdk::Texture) {
        match self.store.write() {
            Ok(mut store) => {
                store.insert(id, texture);
            }
            Err(err) => {
                log::error!("Failed to insert texture on Store: {err:?}");
            }
        }
    }
}
