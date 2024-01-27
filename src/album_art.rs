use std::{
    cell::{OnceCell, RefCell},
    collections::HashMap,
    rc::Rc,
};

use anyhow::{Context, Result};
use futures_util::lock::Mutex;
use gtk::{gdk, glib};
use soup::prelude::*;

// TODO
// - Don't load AlbumArt if network is metered
// - Retry downloading once network is back
// - Integrate more with AlbumCover widget
// - Load only at most n AlbumArt at a time
// - Sanitize the arbitrary data downloaded before converting it to texture

pub struct AlbumArtStore {
    session: soup::Session,
    map: RefCell<HashMap<String, Rc<AlbumArt>>>,
}

impl AlbumArtStore {
    pub fn new(session: soup::Session) -> Self {
        // TODO Remove from store on low memory (Use LRU Cache)

        Self {
            session,
            map: RefCell::default(),
        }
    }

    pub fn get_or_init(&self, download_url: &str) -> Rc<AlbumArt> {
        Rc::clone(
            self.map
                .borrow_mut()
                .entry(download_url.to_string())
                .or_insert_with(|| Rc::new(AlbumArt::new(self.session.clone(), download_url))),
        )
    }
}

pub struct AlbumArt {
    session: soup::Session,
    download_url: String,

    cache: OnceCell<gdk::Texture>,
    cache_guard: Mutex<()>,
}

impl AlbumArt {
    fn new(session: soup::Session, download_url: &str) -> Self {
        Self {
            session,
            download_url: download_url.to_string(),
            cache: OnceCell::new(),
            cache_guard: Mutex::new(()),
        }
    }

    /// Whether the album art is loaded in memory.
    pub fn is_loaded(&self) -> bool {
        self.cache.get().is_some()
    }

    pub fn download_url(&self) -> &str {
        &self.download_url
    }

    pub async fn texture(&self) -> Result<&gdk::Texture> {
        let _guard = self.cache_guard.lock().await;

        if let Some(texture) = self.cache.get() {
            return Ok(texture);
        }

        let bytes = self
            .session
            .send_and_read_future(
                &soup::Message::new("GET", &self.download_url)?,
                glib::Priority::LOW,
            )
            .await
            .context("Failed to download album art bytes")?;
        tracing::trace!(download_url = ?self.download_url, "Downloaded album art bytes");

        let texture = gdk::Texture::from_bytes(&bytes)
            .context("Failed to load album art texture from bytes")?;
        self.cache.set(texture).unwrap();

        Ok(self.cache.get().unwrap())
    }
}

#[cfg(test)]
mod test {
    use super::*;

    use futures_util::future;

    #[gtk::test]
    async fn identity() {
        let store = AlbumArtStore::new(soup::Session::new());
        let download_url =
            "https://www.google.com/images/branding/googlelogo/2x/googlelogo_color_272x92dp.png";
        let access_1 = store.get_or_init(download_url);
        let access_2 = store.get_or_init(download_url);
        assert!(Rc::ptr_eq(&access_1, &access_2));
        assert_eq!(
            access_1.texture().await.unwrap(),
            access_2.texture().await.unwrap()
        );
    }

    #[gtk::test]
    async fn download() {
        let download_url =
            "https://www.google.com/images/branding/googlelogo/2x/googlelogo_color_272x92dp.png";
        let album_art = AlbumArt::new(soup::Session::new(), download_url);
        assert!(!album_art.is_loaded());
        assert_eq!(album_art.download_url(), download_url);

        assert!(album_art.texture().await.is_ok());
        assert!(album_art.is_loaded());

        // Multiple texture call yields the same instance of texture.
        assert_eq!(
            album_art.texture().await.unwrap(),
            album_art.texture().await.unwrap()
        );
    }

    #[gtk::test]
    async fn concurrent_downloads() {
        let download_url =
            "https://www.google.com/images/branding/googlelogo/2x/googlelogo_color_272x92dp.png";
        let album_art = AlbumArt::new(soup::Session::new(), download_url);

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
