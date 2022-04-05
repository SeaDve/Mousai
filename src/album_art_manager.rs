use gtk::{gdk, gio, glib, prelude::*};
use reqwest::Client;

use std::{collections::HashMap, sync::RwLock};

use crate::{
    model::{Song, SongId},
    RUNTIME,
};

pub struct AlbumArtManager {
    store: RwLock<HashMap<SongId, gdk::Texture>>,
    client: reqwest::Client,
}

impl AlbumArtManager {
    pub fn new() -> Self {
        Self {
            store: RwLock::new(HashMap::new()),
            client: Client::new(),
        }
    }

    pub async fn get_or_init(&self, song: &Song) -> anyhow::Result<gdk::Texture> {
        {
            let store = self
                .store
                .read()
                .expect("Failed to read album art store RwLock");
            if let Some(texture) = store.get(&song.id()) {
                return Ok(texture.clone());
            }
        }

        // FIXME more reliable way to create unique cache path
        let cache_path = glib::user_cache_dir().join(song.id().try_to_string()?);

        match gio::File::for_path(&cache_path).load_bytes_future().await {
            Ok((ref bytes, _)) => {
                let texture = gdk::Texture::from_bytes(bytes)?;
                self.try_insert(song.id(), texture.clone());
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
        if let Some(album_art_link) = song.album_art_link() {
            let response = RUNTIME
                .spawn(self.client.get(&album_art_link).send())
                .await??;
            let bytes = RUNTIME.spawn(response.bytes()).await??;
            log::info!("Downloaded album art from link `{album_art_link}`");

            let texture = gdk::Texture::from_bytes(&glib::Bytes::from_owned(bytes))?;
            texture.save_to_png(cache_path)?;

            self.try_insert(song.id(), texture.clone());

            return Ok(texture);
        }

        Err(anyhow::anyhow!(
            "Song doesn't have a provided `album_art_link`"
        ))
    }

    fn try_insert(&self, song_id: SongId, texture: gdk::Texture) {
        match self.store.write() {
            Ok(mut store) => {
                store.insert(song_id, texture);
            }
            Err(err) => {
                log::error!("Failed to write to AlbumArtManager store RwLock: {err:?}");
            }
        }
    }
}
