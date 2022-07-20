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
                    let cache_path = self.cache_path_for_url(download_url);
                    Rc::new(AlbumArt::new(&self.session, download_url, &cache_path))
                }),
        )
    }

    /// Returns always the same path for the same download url.
    fn cache_path_for_url(&self, download_url: &str) -> PathBuf {
        let file_name = download_url.replace('/', "-").replace('\0', "");

        let path = if file_name == "." {
            log::error!("Found download url `.`");
            self.cache_dir.join("dot")
        } else if file_name == ".." {
            log::error!("Found download url `..`");
            self.cache_dir.join("dot-dot")
        } else {
            self.cache_dir.join(file_name)
        };

        // Should be impossible, but to detect it just incase
        if path.file_name().is_none() {
            log::error!("Found no file name for cache path. Defaulting to `album_art`");
        }

        path
    }
}

#[cfg(test)]
mod test {
    use super::*;

    use gtk::gio::prelude::FileExt;

    #[test]
    fn null_bytes() {
        let session = soup::Session::new();
        let store = AlbumArtStore::new(&session).unwrap();

        let album_art = store.get_or_init("http://example.com/albu\0m.jpg");
        assert!(!album_art
            .cache_file()
            .path()
            .unwrap()
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .contains('\0'));
    }

    #[test]
    fn forward_slash() {
        let session = soup::Session::new();
        let store = AlbumArtStore::new(&session).unwrap();

        let album_art = store.get_or_init("http://example.com/album.jpg");
        assert!(!album_art
            .cache_file()
            .path()
            .unwrap()
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .contains('/'));
    }

    #[test]
    fn directory_path() {
        let session = soup::Session::new();
        let store = AlbumArtStore::new(&session).unwrap();

        let album_art = store.get_or_init(".");
        assert!(!album_art.cache_file().path().unwrap().is_dir());

        let album_art = store.get_or_init("..");
        assert!(!album_art.cache_file().path().unwrap().is_dir());
    }
}
