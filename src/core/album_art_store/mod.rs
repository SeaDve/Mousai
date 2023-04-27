mod album_art;

use anyhow::{ensure, Context, Result};
use gtk::glib;

use std::{cell::RefCell, collections::HashMap, fs, path::PathBuf, rc::Rc};

pub use self::album_art::AlbumArt;

pub struct AlbumArtStore {
    session: soup::Session,
    cache_dir: PathBuf,
    inner: RefCell<HashMap<String, Rc<AlbumArt>>>,
}

impl AlbumArtStore {
    /// Initializes the cache dir.
    pub fn new(session: &soup::Session) -> Result<Self> {
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

        // TODO Remove from store on low memory (Use LRU Cache)

        Ok(Self {
            cache_dir,
            session: session.clone(),
            inner: RefCell::default(),
        })
    }

    pub fn get_or_init(&self, download_url: &str) -> Result<Rc<AlbumArt>> {
        use std::collections::hash_map::Entry;

        match self.inner.borrow_mut().entry(download_url.to_string()) {
            Entry::Occupied(entry) => Ok(Rc::clone(entry.get())),
            Entry::Vacant(entry) => {
                // Create a unique cache path for this download URL. Thus escape
                // `/` as it is not allowed in a file name, and remove `\0` as
                // gio::File::for_path() would crash with it.
                let file_name = download_url.replace('/', "-").replace('\0', "");

                ensure!(
                    file_name != "." && file_name != "..",
                    "Download url cannot be `.` or `..`"
                );

                let cache_path = self.cache_dir.join(file_name);

                // Should be practically impossible, but to detect it just incase
                ensure!(
                    cache_path.file_name().is_some(),
                    "Found no file name for created cache path"
                );

                Ok(Rc::clone(entry.insert(Rc::new(AlbumArt::new(
                    &self.session,
                    download_url,
                    &cache_path,
                )))))
            }
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    use gtk::gio::prelude::FileExt;

    #[test]
    fn identity() {
        let session = soup::Session::new();
        let store = AlbumArtStore::new(&session).unwrap();

        let url = "https://example.com/album.jpg";
        let access_1 = store.get_or_init(url).unwrap();
        let access_2 = store.get_or_init(url).unwrap();
        assert_eq!(access_1.cache_file().uri(), access_2.cache_file().uri());
        assert!(Rc::ptr_eq(&access_1, &access_2));
    }

    #[test]
    fn no_null_bytes() {
        let session = soup::Session::new();
        let store = AlbumArtStore::new(&session).unwrap();

        let album_art = store.get_or_init("http://example.com/albu\0m.jpg");
        assert!(!album_art
            .unwrap()
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
    fn no_forward_slashes() {
        let session = soup::Session::new();
        let store = AlbumArtStore::new(&session).unwrap();

        let album_art = store.get_or_init("http://example.com/album.jpg");
        assert!(!album_art
            .unwrap()
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
        assert!(album_art.is_err());

        let album_art = store.get_or_init("..");
        assert!(album_art.is_err());
    }
}
