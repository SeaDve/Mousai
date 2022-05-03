use futures_channel::oneshot::{self, Receiver};
use gtk::{gdk, gio, glib, prelude::*};
use once_cell::{sync::Lazy, unsync::OnceCell};
use soup::prelude::*;

use std::{cell::RefCell, fs, io, path::PathBuf};

use crate::Application;

static CACHE_DIR: Lazy<PathBuf> = Lazy::new(|| {
    let mut path = glib::user_cache_dir();
    path.push("mousai/album_art_cache");
    path
});

#[derive(Debug)]
pub struct AlbumArt {
    download_url: String,
    cache_file: gio::File,
    cache: OnceCell<gdk::Texture>,
    loading: RefCell<Option<Receiver<()>>>,
}

impl AlbumArt {
    pub fn init_cache_dir() -> io::Result<()> {
        fs::create_dir_all(CACHE_DIR.as_path())
    }

    pub fn new(download_url: &str) -> Self {
        // TODO Remove cache on low memory
        let cache_path = CACHE_DIR.join(download_url.to_string().replace('/', "-"));

        Self {
            download_url: download_url.to_string(),
            cache_file: gio::File::for_path(&cache_path),
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
        // TODO Add max loading texture at a certain point of time

        if let Some(receiver) = self.loading.take() {
            // If there are currently loading AlbumArt, wait
            // for it to finish and be stored before checking if
            // it exist. This is to prevent loading the same
            // album art twice on subsequent call on this function.
            let _ = receiver.await;
        }

        if let Some(texture) = self.cache.get() {
            return Ok(texture);
        }

        let (sender, receiver) = oneshot::channel();
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

        let bytes = Application::default()
            .session()
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
        self.cache.set(texture).unwrap();
        self.cache.get().unwrap()
    }
}
