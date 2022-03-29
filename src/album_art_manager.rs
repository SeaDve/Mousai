use gtk::{gdk, glib};
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
            if let Some(paintable) = store.get(&song.id()) {
                return Ok(paintable.clone());
            }
        }

        if let Some(album_art_link) = song.album_art_link() {
            let response = RUNTIME
                .spawn(self.client.get(&album_art_link).send())
                .await??;
            let bytes = RUNTIME.spawn(response.bytes()).await??;
            log::info!("Downloaded album art link from `{album_art_link}`");

            // TODO cache downloads on disk
            let texture = gdk::Texture::from_bytes(&glib::Bytes::from_owned(bytes))?;

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
