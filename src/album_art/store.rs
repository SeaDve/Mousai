use futures_channel::oneshot::{self, Receiver};
use gtk::{
    gdk::{self, prelude::*},
    gio, glib,
};

use reqwest::Client;

use std::{
    collections::HashMap,
    sync::{Mutex, RwLock},
};

use super::AlbumArt;
use crate::{model::SongId, RUNTIME};

#[derive(Default)]
pub struct Store {
    store: RwLock<HashMap<SongId, gdk::Texture>>,
    loading: Mutex<HashMap<SongId, Receiver<()>>>,
    client: Client,
}

impl Store {
    pub async fn get_or_try_load(&self, album_art: &AlbumArt) -> anyhow::Result<gdk::Texture> {
        if let Some(receiver) = self.loading_remove(&album_art.song_id) {
            // If there are currently loading AlbumArt, wait
            // for it to finish and be stored before checking if
            // it exist. This is to prevent loading the same
            // album art twice on subsequent call on this function.
            let _ = receiver.await;
        }

        if let Some(texture) = self.store_get(&album_art.song_id) {
            return Ok(texture);
        }

        let (sender, receiver) = oneshot::channel();
        self.loading_insert(album_art.song_id.clone(), receiver);

        let cache_file = gio::File::for_path(&album_art.cache_path);

        match cache_file.load_bytes_future().await {
            Ok((ref bytes, _)) => {
                let texture = gdk::Texture::from_bytes(bytes)?;
                self.store_insert(album_art.song_id.clone(), texture.clone());
                return Ok(texture);
            }
            Err(err) => log::warn!("Failed to load file from `{}`: {:?}", cache_file.uri(), err),
        }

        let download_url = &album_art.download_url;
        let response = RUNTIME
            .spawn(self.client.get(download_url).send())
            .await??;
        let bytes = RUNTIME.spawn(response.bytes()).await??;
        log::info!("Downloaded album art from link `{download_url}`");

        let texture = gdk::Texture::from_bytes(&glib::Bytes::from_owned(bytes))?;
        self.store_insert(album_art.song_id.clone(), texture.clone());

        let _ = sender.send(());

        let texture_bytes = texture.save_to_png_bytes();
        cache_file
            .replace_contents_future(texture_bytes, None, false, gio::FileCreateFlags::NONE)
            .await
            .map_err(|(_, err)| err)?;

        Ok(texture)
    }

    fn store_get(&self, id: &SongId) -> Option<gdk::Texture> {
        match self.store.read() {
            Ok(store) => store.get(id).cloned(),
            Err(err) => {
                log::error!("Failed to read from store: {err:?}");
                None
            }
        }
    }

    fn store_insert(&self, id: SongId, texture: gdk::Texture) {
        match self.store.write() {
            Ok(mut store) => {
                store.insert(id, texture);
            }
            Err(err) => {
                log::error!("Failed to insert texture on Store: {err:?}");
            }
        }
    }

    fn loading_insert(&self, id: SongId, receiver: Receiver<()>) {
        match self.loading.lock() {
            Ok(mut loading) => {
                loading.insert(id, receiver);
            }
            Err(err) => {
                log::error!("Failed to insert receiver on loading map: {err:?}");
            }
        }
    }

    fn loading_remove(&self, id: &SongId) -> Option<Receiver<()>> {
        match self.loading.lock() {
            Ok(mut loading) => loading.remove(id),
            Err(err) => {
                log::error!("Failed to remove receiver on loading map: {err:?}");
                None
            }
        }
    }
}
