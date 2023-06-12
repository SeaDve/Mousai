mod album_art;

use std::{cell::RefCell, collections::HashMap, rc::Rc};

pub use self::album_art::AlbumArt;

pub struct AlbumArtStore {
    session: soup::Session,
    map: RefCell<HashMap<String, Rc<AlbumArt>>>,
}

impl AlbumArtStore {
    pub fn new(session: &soup::Session) -> Self {
        // TODO Remove from store on low memory (Use LRU Cache)

        Self {
            session: session.clone(),
            map: RefCell::default(),
        }
    }

    pub fn get_or_init(&self, download_url: &str) -> Rc<AlbumArt> {
        Rc::clone(
            self.map
                .borrow_mut()
                .entry(download_url.to_string())
                .or_insert_with(|| Rc::new(AlbumArt::new(&self.session, download_url))),
        )
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn identity() {
        let session = soup::Session::new();
        let store = AlbumArtStore::new(&session);

        let url = "https://example.com/album.jpg";
        let access_1 = store.get_or_init(url);
        let access_2 = store.get_or_init(url);
        assert!(Rc::ptr_eq(&access_1, &access_2));
    }
}
