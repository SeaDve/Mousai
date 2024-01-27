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

    #[gtk::test]
    async fn identity() {
        let session = soup::Session::new();
        let store = AlbumArtStore::new(&session);

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
}
