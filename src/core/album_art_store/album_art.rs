use futures_channel::oneshot::{self, Receiver};
use futures_util::future::{FutureExt, Shared};
use gtk::{gdk, gio, glib, prelude::*};
use once_cell::unsync::OnceCell;
use soup::prelude::*;

use std::{cell::RefCell, path::Path};

pub struct AlbumArt {
    session: soup::Session,
    download_url: String,
    cache_file: gio::File,
    cache: OnceCell<gdk::Texture>,
    loading: RefCell<Option<Shared<Receiver<()>>>>,
}

impl std::fmt::Debug for AlbumArt {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let receiver_strong_count = self
            .loading
            .borrow()
            .as_ref()
            .and_then(|r| r.strong_count())
            .unwrap_or(0);

        f.debug_struct("AlbumArt")
            .field("download_url", &self.download_url)
            .field("cache_uri", &self.cache_file.uri())
            .field("loaded", &self.is_loaded())
            .field("receiver_strong_count", &receiver_strong_count)
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

    /// Whether the album art is loaded in memory.
    pub fn is_loaded(&self) -> bool {
        self.cache.get().is_some()
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
            let _ = receiver.await;
        }

        // Nothing should get passed this point while the
        // AlbumArt is already loading because of the guard above.
        debug_assert!(self.loading.borrow().is_none());

        if let Some(texture) = self.cache.get() {
            return Ok(texture);
        }

        let (sender, receiver) = oneshot::channel();
        self.loading.replace(Some(receiver.shared()));

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

        // Since the cache is already loaded, the receiver to
        // delay consecutive calls to Self::texture is not
        // needed anymore.
        self.loading.replace(None);

        self.cache.get().unwrap()
    }

    #[cfg(test)]
    pub(super) fn cache_file(&self) -> &gio::File {
        &self.cache_file
    }
}

#[cfg(test)]
mod test {
    use super::*;

    use gtk::glib;

    #[test]
    #[serial_test::serial]
    fn download() {
        let session = soup::Session::new();
        let download_url =
            "https://www.google.com/images/branding/googlelogo/2x/googlelogo_color_272x92dp.png";
        let cache_path = glib::tmp_dir().join("image-download.png");

        let album_art = AlbumArt::new(&session, download_url, &cache_path);
        assert!(!album_art.is_loaded());
        assert_eq!(album_art.uri(), download_url);

        glib::MainContext::default().block_on(async move {
            assert!(album_art.texture().await.is_ok());
            assert!(album_art.is_loaded());
            assert_eq!(album_art.uri(), gio::File::for_path(cache_path).uri());

            // Multiple texture call yields the same instance of texture.
            assert_eq!(
                album_art.texture().await.unwrap(),
                album_art.texture().await.unwrap()
            );
        });
    }

    #[test]
    #[serial_test::serial]
    fn concurrent_downloads() {
        let session = soup::Session::new();
        let download_url =
            "https://www.google.com/images/branding/googlelogo/2x/googlelogo_color_272x92dp.png";
        let cache_path = glib::tmp_dir().join("image-concurrent_downloads.png");

        let album_art = AlbumArt::new(&session, download_url, &cache_path);

        glib::MainContext::default().block_on(async move {
            // Should not panic on the following line in `AlbumArt::texture`.
            // debug_assert!(self.loading.borrow().is_none());
            let (res_1, res_2, res_3, res_4) = futures_util::join!(
                album_art.texture(),
                album_art.texture(),
                album_art.texture(),
                album_art.texture(),
            );

            assert!(res_1.is_ok());
            assert!(res_2.is_ok());
            assert!(res_3.is_ok());
            assert!(res_4.is_ok());
        });
    }
}
