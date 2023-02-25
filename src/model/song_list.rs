use anyhow::Result;
use gtk::{
    gio,
    glib::{self, closure_local},
    prelude::*,
    subclass::prelude::*,
};
use indexmap::IndexMap;

use std::{cell::RefCell, collections::HashSet};

use super::{db, Song, SongId};
use crate::utils;

mod imp {
    use super::*;
    use glib::subclass::Signal;
    use once_cell::sync::Lazy;

    #[derive(Debug, Default)]
    pub struct SongList {
        pub(super) list: RefCell<IndexMap<SongId, Song>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for SongList {
        const NAME: &'static str = "MsaiSongList";
        type Type = super::SongList;
        type Interfaces = (gio::ListModel,);
    }

    impl ObjectImpl for SongList {
        fn signals() -> &'static [Signal] {
            static SIGNALS: Lazy<Vec<Signal>> = Lazy::new(|| {
                vec![Signal::builder("removed")
                    .param_types([Song::static_type()])
                    .build()]
            });

            SIGNALS.as_ref()
        }
    }

    impl ListModelImpl for SongList {
        fn item_type(&self) -> glib::Type {
            Song::static_type()
        }

        fn n_items(&self) -> u32 {
            self.list.borrow().len() as u32
        }

        fn item(&self, position: u32) -> Option<glib::Object> {
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
    pub fn load_from_db() -> Result<Self> {
        let this = Self::default();
        this.imp().list.replace(
            db::song::read_all()?
                .into_iter()
                .map(|song| (song.id(), song))
                .collect(),
        );
        Ok(this)
    }

    /// Load a [`SongList`] from application settings `history` key
    pub fn load_from_settings() -> Result<Self> {
        let songs: Vec<Song> = serde_json::from_str(&utils::app_instance().settings().history())?;

        let obj = Self::default();
        obj.append_many(songs);

        Ok(obj)
    }

    /// Save to application settings `history` key
    pub fn save_to_settings(&self) -> Result<()> {
        let list = self.imp().list.borrow();
        let songs = list.values().collect::<Vec<_>>();
        utils::app_instance()
            .settings()
            .try_set_history(&serde_json::to_string(&songs)?)?;
        Ok(())
    }

    /// If an equivalent [`Song`] already exists in the list, it returns false and updates
    /// the original value in the list. Otherwise, it inserts the new [`Song`] at the end and
    /// returns true.
    ///
    /// The equivalence of the [`Song`] depends on its [`SongId`]
    pub fn append(&self, song: Song) -> bool {
        db::song::insert_or_update(&song).unwrap();
        let (position, last_value) = self.imp().list.borrow_mut().insert_full(song.id(), song);

        if last_value.is_some() {
            self.items_changed(position as u32, 1, 1);
            false
        } else {
            self.items_changed(position as u32, 0, 1);
            true
        }
    }

    /// Tries to append all [`Song`]s and returns the number of [`Song`]s that were
    /// actually appended.
    ///
    /// If a [`Song`] is unique to the list, it is appended. Otherwise, the existing
    /// value will be updated.
    ///
    /// This is more efficient than [`SongList::append`] since it emits `items-changed`
    /// only once if all appended [`Song`]s are unique.
    pub fn append_many(&self, songs: Vec<Song>) -> u32 {
        let mut updated_indices = HashSet::new();
        let mut n_appended = 0;

        {
            let mut list = self.imp().list.borrow_mut();

            for song in songs {
                db::song::insert_or_update(&song).unwrap(); // FIXME use batch insert
                let (index, last_value) = list.insert_full(song.id(), song);

                if last_value.is_some() {
                    updated_indices.insert(index);
                } else {
                    n_appended += 1;
                }
            }
        }

        let index_of_first_append = self.n_items() - n_appended;

        // Emit about the appended items first, so GTK would know about
        // the new items and it won't error out because the n_items
        // does not match what GTK expect
        if n_appended != 0 {
            self.items_changed(index_of_first_append, 0, n_appended);
        }

        // This is emitted individually because each updated item
        // may be on different indices
        for index in updated_indices {
            // Only emit if the updated item is before the first appended item
            // because it is already handled by the emission above
            if (index as u32) < index_of_first_append {
                self.items_changed(index as u32, 1, 1);
            }
        }

        n_appended
    }

    pub fn remove(&self, song_id: &SongId) -> Option<Song> {
        let removed = self.imp().list.borrow_mut().shift_remove_full(song_id);

        if let Some((position, _, ref song)) = removed {
            db::song::delete(&song.id()).unwrap();
            self.emit_by_name::<()>("removed", &[song]);
            self.items_changed(position as u32, 1, 0);
        }

        removed.map(|r| r.2)
    }

    pub fn get(&self, song_id: &SongId) -> Option<Song> {
        self.imp().list.borrow().get(song_id).cloned()
    }

    pub fn contains(&self, song_id: &SongId) -> bool {
        self.imp().list.borrow().contains_key(song_id)
    }

    pub fn is_empty(&self) -> bool {
        self.n_items() == 0
    }

    pub fn connect_removed<F>(&self, f: F) -> glib::SignalHandlerId
    where
        F: Fn(&Self, &Song) + 'static,
    {
        self.connect_closure(
            "removed",
            true,
            closure_local!(|obj: &Self, song: &Song| {
                f(obj, song);
            }),
        )
    }
}

impl Default for SongList {
    fn default() -> Self {
        glib::Object::new()
    }
}

#[cfg(test)]
mod test {
    use super::*;

    use std::{
        cell::{Cell, RefCell},
        rc::Rc,
    };

    fn new_test_song(id: &str) -> Song {
        Song::builder(&SongId::new_for_test(id), id, id, id).build()
    }

    #[test]
    fn append_and_remove() {
        let song_list = SongList::default();
        assert!(song_list.is_empty());

        let song_1 = new_test_song("1");
        assert!(song_list.append(song_1.clone()));

        let song_2 = new_test_song("2");
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

        let songs = vec![new_test_song("1"), new_test_song("2")];
        assert_eq!(song_list.append_many(songs), 2);
        assert_eq!(song_list.n_items(), 2);

        let more_songs = vec![new_test_song("SameId"), new_test_song("SameId")];
        assert_eq!(song_list.append_many(more_songs), 1);
        assert_eq!(song_list.n_items(), 3);
    }

    #[test]
    fn items_changed_append() {
        let song_list = SongList::default();

        song_list.connect_items_changed(|_, index, removed, added| {
            assert_eq!(index, 0);
            assert_eq!(removed, 0);
            assert_eq!(added, 1);
        });

        song_list.append(new_test_song("0"));
    }

    #[test]
    fn items_changed_append_index_1() {
        let song_list = SongList::default();
        song_list.append(new_test_song("0"));

        song_list.connect_items_changed(|_, index, removed, added| {
            assert_eq!(index, 1);
            assert_eq!(removed, 0);
            assert_eq!(added, 1);
        });

        song_list.append(new_test_song("1"));
    }

    #[test]
    fn items_changed_append_equal() {
        let song_list = SongList::default();
        song_list.append(new_test_song("0"));

        let n_called = Rc::new(Cell::new(0));

        let n_called_clone = Rc::clone(&n_called);
        song_list.connect_items_changed(move |_, index, removed, added| {
            assert_eq!(index, 0);
            assert_eq!(removed, 1);
            assert_eq!(added, 1);
            n_called_clone.set(n_called_clone.get() + 1);
        });

        assert_eq!(n_called.get(), 0);
        song_list.append(new_test_song("0"));
        assert_eq!(n_called.get(), 1);
    }

    #[test]
    fn items_changed_append_many() {
        let song_list = SongList::default();
        song_list.append(new_test_song("0"));

        song_list.connect_items_changed(|_, index, removed, added| {
            assert_eq!(index, 1);
            assert_eq!(removed, 0);
            assert_eq!(added, 2);
        });

        song_list.append_many(vec![new_test_song("1"), new_test_song("2")]);
    }

    #[test]
    fn items_changed_append_many_with_duplicates() {
        let song_list = SongList::default();
        song_list.append(new_test_song("0"));

        let calls_output = Rc::new(RefCell::new(Vec::new()));

        let calls_output_clone = Rc::clone(&calls_output);
        song_list.connect_items_changed(move |_, index, removed, added| {
            calls_output_clone
                .borrow_mut()
                .push((index, removed, added));
        });

        assert_eq!(
            song_list.append_many(vec![
                new_test_song("0"),
                new_test_song("1"),
                new_test_song("2"),
                new_test_song("2"),
            ]),
            2
        );

        let calls_output = calls_output.borrow();
        assert_eq!(calls_output.len(), 2);
        assert_eq!(calls_output[0], (1, 0, 2));
        assert!(calls_output.contains(&(0, 1, 1)));
    }

    #[test]
    fn items_changed_append_many_more_removed_than_n_items() {
        let song_list = SongList::default();
        song_list.append(new_test_song("0"));
        song_list.append(new_test_song("1"));

        let calls_output = Rc::new(RefCell::new(Vec::new()));

        let calls_output_clone = Rc::clone(&calls_output);
        song_list.connect_items_changed(move |_, index, removed, added| {
            calls_output_clone
                .borrow_mut()
                .push((index, removed, added));
        });

        assert_eq!(
            song_list.append_many(vec![
                new_test_song("0"),
                new_test_song("0"),
                new_test_song("0"),
                new_test_song("1"),
                new_test_song("2"),
            ]),
            1
        );

        let calls_output = calls_output.borrow();
        assert_eq!(calls_output.len(), 3);
        assert_eq!(calls_output[0], (2, 0, 1));
        assert!(calls_output.contains(&(0, 1, 1)));
        assert!(calls_output.contains(&(1, 1, 1)));
    }

    #[test]
    fn items_changed_removed_some() {
        let song_list = SongList::default();
        song_list.append(new_test_song("0"));

        let n_called = Rc::new(Cell::new(0));

        let n_called_clone = Rc::clone(&n_called);
        song_list.connect_items_changed(move |_, index, removed, added| {
            assert_eq!(index, 0);
            assert_eq!(removed, 1);
            assert_eq!(added, 0);
            n_called_clone.set(n_called_clone.get() + 1);
        });

        assert_eq!(n_called.get(), 0);
        assert_eq!(
            song_list.remove(&SongId::new_for_test("0")).unwrap().id(),
            SongId::new_for_test("0")
        );
        assert_eq!(n_called.get(), 1);
    }

    #[test]
    fn items_changed_removed_none() {
        let song_list = SongList::default();

        let n_called = Rc::new(Cell::new(0));

        let n_called_clone = Rc::clone(&n_called);
        song_list.connect_items_changed(move |_, index, removed, added| {
            assert_eq!(index, 0);
            assert_eq!(removed, 0);
            assert_eq!(added, 0);
            n_called_clone.set(n_called_clone.get() + 1);
        });

        assert_eq!(n_called.get(), 0);
        assert!(song_list.remove(&SongId::new_for_test("0")).is_none());
        assert_eq!(n_called.get(), 0);
    }

    #[test]
    fn connect_removed_some() {
        let song_list = SongList::default();
        song_list.append(new_test_song("0"));

        let n_called = Rc::new(Cell::new(0));

        let n_called_clone = Rc::clone(&n_called);
        song_list.connect_removed(move |_, song| {
            assert_eq!(song.id(), SongId::new_for_test("0"));
            n_called_clone.set(n_called_clone.get() + 1);
        });

        assert_eq!(n_called.get(), 0);
        assert_eq!(
            song_list.remove(&SongId::new_for_test("0")).unwrap().id(),
            SongId::new_for_test("0")
        );
        assert_eq!(n_called.get(), 1);
    }

    #[test]
    fn connect_removed_none() {
        let song_list = SongList::default();

        let n_called = Rc::new(Cell::new(0));

        let n_called_clone = Rc::clone(&n_called);
        song_list.connect_removed(move |_, _| {
            n_called_clone.set(n_called_clone.get() + 1);
        });

        assert_eq!(n_called.get(), 0);
        assert!(song_list.remove(&SongId::new_for_test("0")).is_none());
        assert_eq!(n_called.get(), 0);
    }
}
