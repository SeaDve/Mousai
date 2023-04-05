use anyhow::Result;
use futures_channel::oneshot::{self, Receiver, Sender};
use futures_util::future::{FutureExt, Shared};
use gtk::{gdk, gio, glib, prelude::*};
use once_cell::unsync::OnceCell;
use soup::prelude::*;

use std::{cell::RefCell, fmt, path::Path};

use crate::debug_assert_or_log;

// TODO
// - Don't load AlbumArt if network is metered
// - Retry downloading once network is back
// - Integrate more with AlbumCover widget
// - Load only at most n AlbumArt at a time
// - Sanitize the arbitrary data downloaded before converting it to texture

pub struct AlbumArt {
    session: soup::Session,
    download_url: String,
    cache_file: gio::File,
    cache: OnceCell<gdk::Texture>,
    #[allow(clippy::type_complexity)]
    guard: RefCell<Option<(Sender<()>, Shared<Receiver<()>>)>>,
}

impl fmt::Debug for AlbumArt {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let rx_strong_count = self
            .guard
            .borrow()
            .as_ref()
            .and_then(|(_, rx)| rx.strong_count())
            .unwrap_or(0);

        f.debug_struct("AlbumArt")
            .field("download_url", &self.download_url)
            .field("cache_uri", &self.cache_file.uri())
            .field("loaded", &self.is_loaded())
            .field("rx_strong_count", &rx_strong_count)
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
            guard: RefCell::default(),
        }
    }

    /// Whether the album art is loaded in memory.
    pub fn is_loaded(&self) -> bool {
        self.cache.get().is_some()
    }

    pub fn uri(&self) -> glib::GString {
        if self.cache_file.query_exists(gio::Cancellable::NONE) {
            return self.cache_file.uri();
        }

        self.download_url.as_str().into()
    }

    pub async fn texture(&self) -> Result<&gdk::Texture> {
        let rx = self.guard.borrow().as_ref().map(|(_, rx)| rx.clone());
        if let Some(rx) = rx {
            // If there are currently loading AlbumArt, wait
            // for it to finish and be stored before checking if
            // it exist. This is to prevent loading the same
            // album art twice on subsequent call on this function.
            let _ = rx.await;
        }

        // Nothing should get passed this point while the
        // AlbumArt is already loading because of the guard above.
        debug_assert_or_log!(self.guard.borrow().is_none());

        if let Some(texture) = self.cache.get() {
            return Ok(texture);
        }

        let (tx, rx) = oneshot::channel();
        self.guard.replace(Some((tx, rx.shared())));

        match self.cache_file.load_bytes_future().await {
            Ok((ref bytes, _)) => {
                let texture = gdk::Texture::from_bytes(bytes)?;
                return Ok(self.set_and_get_cache(texture));
            }
            Err(err) => {
                if err.matches(gio::IOErrorEnum::NotFound) {
                    tracing::debug!(
                        uri = ?self.cache_file.uri(),
                        "Cache file not found; downloading album art",
                    );
                } else {
                    return Err(err.into());
                }
            }
        }

        let bytes = self
            .session
            .send_and_read_future(
                &soup::Message::new("GET", &self.download_url)?,
                glib::PRIORITY_LOW,
            )
            .await?;
        tracing::debug!(download_url = ?self.download_url, "Downloaded album art");

        let texture = self.set_and_get_cache(gdk::Texture::from_bytes(&bytes)?);

        let texture_bytes = texture.save_to_png_bytes();
        self.cache_file
            .replace_contents_future(texture_bytes, None, false, gio::FileCreateFlags::NONE)
            .await
            .map_err(|(_, err)| err)?;

        Ok(texture)
    }

    fn set_and_get_cache(&self, texture: gdk::Texture) -> &gdk::Texture {
        let ret = match self.cache.try_insert(texture) {
            Ok(final_value) => final_value,
            Err((final_value, texture)) => {
                tracing::error!(
                    "cache was already set; is_same_instance = {}",
                    final_value == &texture,
                );
                final_value
            }
        };

        // Since the cache is already loaded, the guard to
        // delay consecutive calls to Self::texture is not
        // needed anymore.
        self.guard.replace(None);

        ret
    }

    #[cfg(test)]
    pub(super) fn cache_file(&self) -> &gio::File {
        &self.cache_file
    }
}

#[cfg(test)]
mod test {
    use super::*;

    use futures_util::future;

    #[gtk::test]
    async fn download() {
        let session = soup::Session::new();
        let download_url =
            "https://www.google.com/images/branding/googlelogo/2x/googlelogo_color_272x92dp.png";

        let tempdir = tempfile::tempdir().unwrap();
        let cache_path = tempdir.path().join("image-download.png");

        let album_art = AlbumArt::new(&session, download_url, &cache_path);
        assert!(!album_art.is_loaded());
        assert_eq!(album_art.uri(), download_url);

        assert!(album_art.texture().await.is_ok());
        assert!(album_art.is_loaded());
        assert_eq!(
            album_art.uri(),
            glib::filename_to_uri(&cache_path, None).unwrap()
        );

        // Multiple texture call yields the same instance of texture.
        assert_eq!(
            album_art.texture().await.unwrap(),
            album_art.texture().await.unwrap()
        );
    }

    #[gtk::test]
    async fn concurrent_downloads() {
        let session = soup::Session::new();
        let download_url =
            "https://www.google.com/images/branding/googlelogo/2x/googlelogo_color_272x92dp.png";

        let tempdir = tempfile::tempdir().unwrap();
        let cache_path = tempdir.path().join("image-concurrent_downloads.png");

        let album_art = AlbumArt::new(&session, download_url, &cache_path);

        // Should not panic on the following line in `AlbumArt::texture`.
        // debug_assert!(self.loading.borrow().is_none());
        let results = future::join_all(vec![
            album_art.texture(),
            album_art.texture(),
            album_art.texture(),
            album_art.texture(),
        ])
        .await;

        assert!(results
            .iter()
            .all(|r| r.as_ref().unwrap() == results[0].as_ref().unwrap()));
    }
}
