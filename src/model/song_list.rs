use adw::subclass::prelude::*;
use gtk::{gio, glib, prelude::*};
use indexmap::IndexMap;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use std::cell::RefCell;

use crate::{core::DateTime, Application};

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
    /// Load a [`SongList`](SongList) from application settings `history` key
    pub fn load_from_settings() -> anyhow::Result<Self> {
        let json_str = Application::default().settings().string("history");
        Ok(serde_json::from_str(&json_str)?)
    }

    /// Save to application settings `history` key
    pub fn save_to_settings(&self) -> anyhow::Result<()> {
        let json_str = serde_json::to_string(&self)?;
        Application::default()
            .settings()
            .set_string("history", &json_str)?;
        Ok(())
    }

    /// If an equivalent [`Song`] already exists in the list, it returns false updating the original
    /// value in the list. Otherwise, it inserts the new [`Song`] at the end and returns true.
    ///
    /// Update the [`Song`]'s `last-heard` value when the song already exist.
    ///
    /// The equivalence of the [`Song`] depends on their [`SongId`]
    pub fn append(&self, song: Song) -> bool {
        let song_clone = song.clone();

        let (position, last_value) = self.imp().list.borrow_mut().insert_full(song.id(), song);

        if last_value.is_some() {
            // FIXME handle this outside this function
            song_clone.set_last_heard(DateTime::now());
            self.items_changed(position as u32, 1, 1);
            return false;
        }

        self.items_changed(position as u32, 0, 1);

        true
    }

    /// It tries to append all [`Song`]s. When any of the song already exist, it returns false
    /// leaving the original value of the existing [`Song`]s. If all [`Song`]s are unique, it
    /// returns true.
    ///
    /// This is more efficient than [`SongList::append`] since it emits `items-changed` only once
    pub fn append_many(&self, songs: Vec<Song>) -> bool {
        let initial_songs_len = songs.len();

        let mut n_appended = 0;

        {
            let mut list = self.imp().list.borrow_mut();

            for song in songs {
                if list.insert(song.id(), song).is_none() {
                    n_appended += 1;
                }
            }
        }

        if n_appended > 0 {
            self.items_changed(self.n_items() - 1, 0, n_appended);
        }

        n_appended as usize == initial_songs_len
    }

    pub fn remove(&self, song_id: &SongId) -> Option<Song> {
        let removed = self.imp().list.borrow_mut().shift_remove_full(song_id);

        if let Some((position, _, _)) = removed {
            self.items_changed(position as u32, 1, 0);
        }

        removed.map(|r| r.2)
    }

    pub fn get(&self, note_id: &SongId) -> Option<Song> {
        self.imp().list.borrow().get(note_id).cloned()
    }

    pub fn is_empty(&self) -> bool {
        self.n_items() == 0
    }
}

impl Default for SongList {
    fn default() -> Self {
        glib::Object::new(&[]).expect("Failed to create SongList.")
    }
}

impl Serialize for SongList {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_seq(self.imp().list.borrow().values())
    }
}

impl<'de> Deserialize<'de> for SongList {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let songs: Vec<Song> = Vec::deserialize(deserializer)?;

        let song_list = SongList::default();
        song_list.append_many(songs);

        Ok(song_list)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn append_and_remove() {
        let song_list = SongList::default();
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
        let song_list = SongList::default();
        assert!(song_list.is_empty());

        let songs = vec![Song::new("1", "1", "1"), Song::new("2", "2", "2")];
        assert!(song_list.append_many(songs));
        assert_eq!(song_list.n_items(), 2);

        let more_songs = vec![
            Song::new("", "", "SameInfoLink"),
            Song::new("", "", "SameInfoLink"),
        ];
        assert!(!song_list.append_many(more_songs));
        assert_eq!(song_list.n_items(), 3);
    }
}
