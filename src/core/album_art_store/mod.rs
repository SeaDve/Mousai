mod album_art;

use futures_channel::oneshot::{self, Receiver};
use gtk::{
    gdk::{self, prelude::*},
    gio, glib,
};
use soup::prelude::*;

use std::{
    cell::RefCell,
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    rc::Rc,
};

pub use self::album_art::AlbumArt;

#[derive(Debug)]
pub struct AlbumArtStore(Rc<AlbumArtStoreInner>);

#[derive(Debug)]
struct AlbumArtStoreInner {
    session: soup::Session,
    store: RefCell<HashMap<String, gdk::Texture>>,
    loading: RefCell<HashMap<String, Receiver<()>>>,
    cache_dir: PathBuf,
}

impl AlbumArtStoreInner {
    async fn get_or_try_load_texture(
        &self,
        download_url: &str,
        cache_path: &Path,
    ) -> anyhow::Result<gdk::Texture> {
        let receiver = { self.loading.borrow_mut().remove(download_url) };
        if let Some(receiver) = receiver {
            // If there are currently loading AlbumArt, wait
            // for it to finish and be stored before checking if
            // it exist. This is to prevent loading the same
            // album art twice on subsequent call on this function.
            let _ = receiver.await;
        }

        // TODO Add max loading texture at a certain point of time

        if let Some(texture) = self.store.borrow().get(download_url) {
            return Ok(texture.clone());
        }

        let (sender, receiver) = oneshot::channel();
        self.loading
            .borrow_mut()
            .insert(download_url.to_string(), receiver);

        let cache_file = gio::File::for_path(&cache_path);

        match cache_file.load_bytes_future().await {
            Ok((ref bytes, _)) => {
                let texture = gdk::Texture::from_bytes(bytes)?;
                self.store
                    .borrow_mut()
                    .insert(download_url.to_string(), texture.clone());
                return Ok(texture);
            }
            Err(err) => log::warn!("Failed to load file from `{}`: {:?}", cache_file.uri(), err),
        }

        let bytes = self
            .session
            .send_and_read_future(
                &soup::Message::new("GET", download_url)?,
                glib::PRIORITY_DEFAULT,
            )
            .await?;
        log::info!("Downloaded album art from link `{download_url}`");

        let texture = gdk::Texture::from_bytes(&bytes)?;
        self.store
            .borrow_mut()
            .insert(download_url.to_string(), texture.clone());

        let _ = sender.send(());

        let texture_bytes = texture.save_to_png_bytes();
        cache_file
            .replace_contents_future(texture_bytes, None, false, gio::FileCreateFlags::NONE)
            .await
            .map_err(|(_, err)| err)?;

        Ok(texture)
    }

    fn cache_dir(&self) -> &Path {
        self.cache_dir.as_path()
    }
}

impl AlbumArtStore {
    /// This also initialize this cache dir.
    pub fn new(session: soup::Session) -> anyhow::Result<Self> {
        let cache_dir = {
            let mut path = glib::user_cache_dir();
            path.push("mousai/album_art_cache");
            path
        };

        fs::create_dir_all(&cache_dir)?;

        // TODO Remove cache on low memory

        Ok(Self(Rc::new(AlbumArtStoreInner {
            session,
            store: RefCell::default(),
            loading: RefCell::default(),
            cache_dir,
        })))
    }

    pub fn get(&self, download_url: &str) -> AlbumArt {
        AlbumArt::new(download_url, &self.0)
    }
}
