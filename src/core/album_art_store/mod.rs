mod album_art;

use anyhow::Context;
use gtk::glib;

use std::{cell::RefCell, collections::HashMap, fs, path::PathBuf, rc::Rc};

pub use self::album_art::AlbumArt;

#[derive(Debug)]
pub struct AlbumArtStore {
    session: soup::Session,
    cache_dir: PathBuf,
    inner: RefCell<HashMap<String, Rc<AlbumArt>>>,
}

impl AlbumArtStore {
    /// Initializes the cache dir.
    pub fn new(session: &soup::Session) -> anyhow::Result<Self> {
        let cache_dir = {
            let mut path = glib::user_cache_dir();
            path.push("mousai/album_art_cache");
            path
        };

        fs::create_dir_all(&cache_dir).with_context(|| {
            format!(
                "Failed to create AlbumArt cache dir at `{}`",
                cache_dir.display()
            )
        })?;

        // TODO Remove from store on low memory

        Ok(Self {
            cache_dir,
            session: session.clone(),
            inner: RefCell::default(),
        })
    }

    pub fn get_or_init(&self, download_url: &str) -> Rc<AlbumArt> {
        Rc::clone(
            self.inner
                .borrow_mut()
                .entry(download_url.to_string())
                .or_insert_with_key(|download_url| {
                    let cache_path = self.cache_dir.join(download_url.replace('/', "-"));
                    Rc::new(AlbumArt::new(&self.session, download_url, &cache_path))
                }),
        )
    }
}
