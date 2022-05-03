mod album_art;

use gtk::glib;
use once_cell::sync::Lazy;

use std::{cell::RefCell, collections::HashMap, fs, io, path::PathBuf, rc::Rc};

pub use self::album_art::AlbumArt;

static CACHE_DIR: Lazy<PathBuf> = Lazy::new(|| {
    let mut path = glib::user_cache_dir();
    path.push("mousai/album_art_cache");
    path
});

#[derive(Debug)]
pub struct AlbumArtStore {
    session: soup::Session,
    inner: RefCell<HashMap<String, Rc<AlbumArt>>>,
}

impl AlbumArtStore {
    pub fn new(session: &soup::Session) -> io::Result<Self> {
        fs::create_dir_all(CACHE_DIR.as_path())?;

        Ok(Self {
            session: session.clone(),
            inner: RefCell::default(),
        })
    }

    pub fn get_or_try_init(&self, download_url: &str) -> Rc<AlbumArt> {
        Rc::clone(
            self.inner
                .borrow_mut()
                .entry(download_url.to_string())
                .or_insert_with_key(|download_url| {
                    let cache_path = CACHE_DIR.join(download_url.to_string().replace('/', "-"));
                    Rc::new(AlbumArt::new(&self.session, download_url, &cache_path))
                }),
        )
    }
}
