use adw::subclass::prelude::*;
use gtk::{gio, glib, prelude::*};
use indexmap::IndexMap;

use std::cell::RefCell;

use super::{Song, SongId};

mod imp {
    use super::*;

    #[derive(Debug, Default)]
    pub struct SongList {
        pub list: RefCell<IndexMap<SongId, Song>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for SongList {
        const NAME: &'static str = "MsaiSongList";
        type Type = super::SongList;
        type ParentType = glib::Object;
        type Interfaces = (gio::ListModel,);
    }

    impl ObjectImpl for SongList {}

    impl ListModelImpl for SongList {
        fn item_type(&self, _list_model: &Self::Type) -> glib::Type {
            Song::static_type()
        }

        fn n_items(&self, _list_model: &Self::Type) -> u32 {
            self.list.borrow().len() as u32
        }

        fn item(&self, _list_model: &Self::Type, position: u32) -> Option<glib::Object> {
            self.list
                .borrow()
                .get_index(position as usize)
                .map(|(_, v)| v.upcast_ref::<glib::Object>())
                .cloned()
        }
    }
}

glib::wrapper! {
    pub struct SongList(ObjectSubclass<imp::SongList>)
        @implements gio::ListModel;
}

impl SongList {
    pub fn new() -> Self {
        glib::Object::new(&[]).expect("Failed to create SongList.")
    }

    /// If an equivalent [`Song`] already exists in the list, it returns false leaving the original
    /// value in the list. Otherwise, it inserts the new [`Song`] and returns true.
    ///
    /// The equivalence of the [`Song`] depends on their [`SongId`]
    pub fn append(&self, song: Song) -> bool {
        let imp = imp::SongList::from_instance(self);

        let is_appended = imp.list.borrow_mut().insert(song.id(), song).is_none();

        if is_appended {
            self.items_changed(self.n_items() - 1, 0, 1);
        }

        is_appended
    }

    /// It tries to append all [`Song`]s. When any of the song already exist, it returns false
    /// leaving the original value of the existing [`Song`]s. If all [`Song`]s are unique, it
    /// returns true.
    ///
    /// This is more efficient than [`SongList::append`] since it emits `items-changed` only once
    pub fn append_many(&self, songs: &[Song]) -> bool {
        let imp = imp::SongList::from_instance(self);

        let mut appended = 0;

        {
            let mut list = imp.list.borrow_mut();

            for song in songs {
                if list.insert(song.id(), song.clone()).is_none() {
                    appended += 1;
                }
            }
        }

        self.items_changed(self.n_items() - 1, 0, appended);

        appended as usize == songs.len()
    }

    pub fn remove(&self, song_id: &SongId) -> Option<Song> {
        let imp = imp::SongList::from_instance(self);

        let removed = imp.list.borrow_mut().shift_remove_full(song_id);

        if let Some((position, _, _)) = removed {
            self.items_changed(position as u32, 1, 0);
        }

        removed.map(|r| r.2)
    }

    pub fn get(&self, note_id: &SongId) -> Option<Song> {
        let imp = imp::SongList::from_instance(self);
        imp.list.borrow().get(note_id).cloned()
    }

    pub fn is_empty(&self) -> bool {
        self.n_items() == 0
    }
}

impl Default for SongList {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn append_and_remove() {
        let song_list = SongList::new();
        assert!(song_list.is_empty());

        let song_1 = Song::new("1", "1", "1");
        assert!(song_list.append(song_1.clone()));

        let song_2 = Song::new("2", "2", "2");
        assert!(song_list.append(song_2.clone()));

        assert!(!song_list.is_empty());
        assert_eq!(song_list.n_items(), 2);

        assert_eq!(song_list.get(&song_1.id()), Some(song_1.clone()));
        assert_eq!(song_list.get(&song_2.id()), Some(song_2.clone()));

        assert_eq!(song_list.remove(&song_1.id()), Some(song_1));
        assert_eq!(song_list.remove(&song_2.id()), Some(song_2));

        assert!(song_list.is_empty());
    }

    #[test]
    fn append_many() {
        let song_list = SongList::new();
        assert!(song_list.is_empty());

        let songs = [Song::new("1", "1", "1"), Song::new("2", "2", "2")];
        assert!(song_list.append_many(&songs));
        assert_eq!(song_list.n_items(), 2);

        let more_songs = [
            Song::new("", "", "SameInfoLink"),
            Song::new("", "", "SameInfoLink"),
        ];
        assert!(!song_list.append_many(&more_songs));
        assert_eq!(song_list.n_items(), 3);
    }
}
