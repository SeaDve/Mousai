use async_channel::Receiver;
use gtk::{gdk, gio, glib, prelude::*};
use once_cell::unsync::OnceCell;
use soup::prelude::*;

use std::{cell::RefCell, path::Path};

pub struct AlbumArt {
    session: soup::Session,
    download_url: String,
    cache_file: gio::File,
    cache: OnceCell<gdk::Texture>,
    loading: RefCell<Option<Receiver<()>>>,
}

impl std::fmt::Debug for AlbumArt {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let is_receiver_closed = if let Some(ref receiver) = *self.loading.borrow() {
            receiver.is_closed()
        } else {
            true
        };

        let n_receiver = if let Some(ref receiver) = *self.loading.borrow() {
            receiver.receiver_count()
        } else {
            0
        };

        f.debug_struct("AlbumArt")
            .field("download_url", &self.download_url)
            .field("cache_uri", &self.cache_file.uri())
            .field("loaded", &self.cache.get().is_some())
            .field("receiver_closed", &is_receiver_closed)
            .field("n_receiver", &n_receiver)
            .finish()
    }
}

impl AlbumArt {
    pub(super) fn new(session: &soup::Session, download_url: &str, cache_path: &Path) -> Self {
        Self {
            session: session.clone(),
            download_url: download_url.to_string(),
            cache_file: gio::File::for_path(cache_path),
            cache: OnceCell::new(),
            loading: RefCell::default(),
        }
    }

    pub fn uri(&self) -> String {
        if self.cache_file.query_exists(gio::Cancellable::NONE) {
            return self.cache_file.uri().into();
        }

        self.download_url.clone()
    }

    pub async fn texture(&self) -> anyhow::Result<&gdk::Texture> {
        let receiver = self.loading.borrow().clone();
        if let Some(receiver) = receiver {
            // If there are currently loading AlbumArt, wait
            // for it to finish and be stored before checking if
            // it exist. This is to prevent loading the same
            // album art twice on subsequent call on this function.
            let _ = receiver.recv().await;
        }

        if let Some(texture) = self.cache.get() {
            return Ok(texture);
        }

        let (sender, receiver) = async_channel::unbounded();
        self.loading.replace(Some(receiver));

        match self.cache_file.load_bytes_future().await {
            Ok((ref bytes, _)) => {
                let texture = gdk::Texture::from_bytes(bytes)?;
                return Ok(self.set_and_get_cache(texture));
            }
            Err(err) => log::warn!(
                "Failed to load file from `{}`: {:?}",
                self.cache_file.uri(),
                err
            ),
        }

        let bytes = self
            .session
            .send_and_read_future(
                &soup::Message::new("GET", &self.download_url)?,
                glib::PRIORITY_DEFAULT,
            )
            .await?;
        log::info!("Downloaded album art from link `{}`", self.download_url);

        let texture = self.set_and_get_cache(gdk::Texture::from_bytes(&bytes)?);

        let _ = sender.send(());

        let texture_bytes = texture.save_to_png_bytes();
        self.cache_file
            .replace_contents_future(texture_bytes, None, false, gio::FileCreateFlags::NONE)
            .await
            .map_err(|(_, err)| err)?;

        Ok(texture)
    }

    fn set_and_get_cache(&self, texture: gdk::Texture) -> &gdk::Texture {
        if let Err(texture) = self.cache.set(texture) {
            log::error!(
                "Cache was already set; is_same_instance = {}",
                &texture == self.cache.get().unwrap()
            );
        }

        self.cache.get().unwrap()
    }
}
