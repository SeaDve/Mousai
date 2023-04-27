use anyhow::{Context, Result};
use futures_util::lock::Mutex;
use gtk::{gdk, gio, glib, prelude::*};
use once_cell::unsync::OnceCell;
use soup::prelude::*;

use std::path::Path;

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
    cache_guard: Mutex<()>,
}

impl AlbumArt {
    pub(super) fn new(session: &soup::Session, download_url: &str, cache_path: &Path) -> Self {
        Self {
            session: session.clone(),
            download_url: download_url.to_string(),
            cache_file: gio::File::for_path(cache_path),
            cache: OnceCell::new(),
            cache_guard: Mutex::new(()),
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

    pub async fn texture(&self) -> Result<&gdk::Texture> {
        let guard = self.cache_guard.lock().await;

        if let Some(texture) = self.cache.get() {
            return Ok(texture);
        }

        match self.cache_file.load_bytes_future().await {
            Ok((ref bytes, _)) => {
                let texture = gdk::Texture::from_bytes(bytes)
                    .context("Failed to load album art texture from bytes")?;
                return Ok(self.set_and_get_cache(texture));
            }
            Err(err) => {
                if !err.matches(gio::IOErrorEnum::NotFound) {
                    return Err(err).context("Failed to load album art bytes from cache")?;
                }

                tracing::debug!(
                    uri = ?self.cache_file.uri(),
                    "Cache file not found; downloading album art",
                );
            }
        }

        let bytes = self
            .session
            .send_and_read_future(
                &soup::Message::new("GET", &self.download_url)?,
                glib::PRIORITY_LOW,
            )
            .await
            .context("Failed to download album art bytes")?;
        tracing::debug!(download_url = ?self.download_url, "Downloaded album art bytes");

        let texture = gdk::Texture::from_bytes(&bytes)
            .context("Failed to load album art texture from bytes")?;
        let texture = self.set_and_get_cache(texture);

        // We don't need to hold the lock anymore since this function would
        // early return even if we are still saving the texture to disk.
        drop(guard);

        let png_bytes = texture.save_to_png_bytes();
        self.cache_file
            .replace_contents_future(png_bytes, None, false, gio::FileCreateFlags::NONE)
            .await
            .map_err(|(_, err)| err)
            .context("Failed to save album art texture to cache file")?;
        tracing::debug!(uri = ?self.cache_file.uri(), "Saved album art to cache file");

        Ok(texture)
    }

    fn set_and_get_cache(&self, texture: gdk::Texture) -> &gdk::Texture {
        match self.cache.try_insert(texture) {
            Ok(final_value) => final_value,
            Err((final_value, texture)) => {
                unreachable!(
                    "cache must not already be set; is_same_instance = {}",
                    final_value == &texture,
                );
            }
        }
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
        // debug_assert!(self.guard.borrow().is_none());
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
