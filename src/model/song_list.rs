use anyhow::Result;
use gtk::{
    gio,
    glib::{self, clone, closure_local},
    prelude::*,
    subclass::prelude::*,
};
use indexmap::IndexMap;
use once_cell::unsync::OnceCell;

use std::{cell::RefCell, collections::HashSet};

use super::{Song, SongId};
use crate::core::{Database, DatabaseError, DatabaseTable};

const SONG_NOTIFY_HANDLER_ID_KEY: &str = "mousai-song-notify-handler-id";

mod imp {
    use super::*;
    use glib::subclass::Signal;
    use once_cell::sync::Lazy;

    #[derive(Debug, Default)]
    pub struct SongList {
        pub(super) list: RefCell<IndexMap<SongId, Song>>,

        pub(super) db_table: OnceCell<DatabaseTable<Song>>,
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
    /// Load from the `songs` table in the database
    pub fn load_from_db(db: &Database) -> Result<Self> {
        let db_table = db.table::<Song>("songs")?;

        let songs = db_table
            .select_all()?
            .into_values()
            .map(|song| (song.id(), song))
            .collect::<IndexMap<_, _>>();

        let this = glib::Object::new::<Self>();

        for (_, song) in songs.iter() {
            this.bind_song_to_db(song);
        }

        this.imp().list.replace(songs);
        this.imp().db_table.set(db_table).unwrap();

        Ok(this)
    }

    /// If an equivalent [`Song`] already exists in the list, it returns false and updates
    /// the original value in the list. Otherwise, it inserts the new [`Song`] at the end and
    /// returns true.
    ///
    /// The equivalence of the [`Song`] depends on its [`SongId`]
    pub fn append(&self, song: Song) -> bool {
        self.db_table()
            .upsert_one(song.id_ref().as_str(), &song)
            .unwrap();

        self.bind_song_to_db(&song);
        let (position, last_value) = self.imp().list.borrow_mut().insert_full(song.id(), song);

        if let Some(last_value) = last_value {
            unbind_song_to_db(&last_value);
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
        self.db_table()
            .upsert_many(
                songs
                    .iter()
                    .map(|song| (song.id_ref().as_str(), song))
                    .collect::<Vec<_>>(),
            )
            .unwrap();

        let mut updated_indices = HashSet::new();
        let mut n_appended = 0;

        {
            let mut list = self.imp().list.borrow_mut();

            for song in songs {
                self.bind_song_to_db(&song);
                let (index, last_value) = list.insert_full(song.id(), song);

                if let Some(last_value) = last_value {
                    unbind_song_to_db(&last_value);
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
        match self.db_table().delete_one(song_id.as_str()) {
            Ok(_) => {}
            Err(err) if matches!(err, DatabaseError::NotFound) => {}
            Err(err) => panic!("Failed to remove song from database: {:?}", err),
        }

        let removed = self.imp().list.borrow_mut().shift_remove_full(song_id);

        if let Some((position, _, ref song)) = removed {
            unbind_song_to_db(song);
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

    fn db_table(&self) -> &DatabaseTable<Song> {
        self.imp().db_table.get().unwrap()
    }

    fn bind_song_to_db(&self, song: &Song) {
        unsafe {
            let handler_id = song.connect_notify_local(
                None,
                clone!(@weak self as obj => move |song, _| {
                    obj.db_table()
                        .update_one(song.id_ref().as_str(), song)
                        .unwrap();
                }),
            );
            song.set_data(SONG_NOTIFY_HANDLER_ID_KEY, handler_id);
        }
    }
}

fn unbind_song_to_db(song: &Song) {
    unsafe {
        let handler_id = song
            .steal_data::<glib::SignalHandlerId>(SONG_NOTIFY_HANDLER_ID_KEY)
            .unwrap();
        song.disconnect(handler_id);
    };
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

    fn new_test_song_list() -> SongList {
        SongList::load_from_db(&Database::open_in_memory().unwrap()).unwrap()
    }

    #[track_caller]
    fn assert_n_items_and_db_count_eq(song_list: &SongList, n: usize) {
        assert_eq!(song_list.n_items(), n as u32);
        assert_eq!(song_list.db_table().count().unwrap(), n);
    }

    /// Must have exactly 2 songs
    fn assert_synced_to_db(song_list: &SongList) {
        let table_items = song_list.db_table().select_all().unwrap();
        assert_eq!(table_items.len(), 2);

        // Test if the items are synced to the database
        for (id, song) in song_list.imp().list.borrow().iter() {
            assert_eq!(
                table_items.get(id.as_str()).unwrap().is_newly_heard(),
                song.is_newly_heard()
            );
        }

        for (_, song) in song_list.db_table().select_all().unwrap() {
            assert!(!song.is_newly_heard());
        }

        {
            song_list
                .item(0)
                .and_downcast::<Song>()
                .unwrap()
                .set_is_newly_heard(true);

            let table_items = song_list.db_table().select_all().unwrap();
            assert_eq!(table_items.len(), 2);

            // Test if the items are synced to the database even
            // after the song is modified
            for (id, song) in song_list.imp().list.borrow().iter() {
                assert_eq!(
                    table_items.get(id.as_str()).unwrap().is_newly_heard(),
                    song.is_newly_heard()
                );
            }
        }

        {
            song_list
                .item(1)
                .and_downcast::<Song>()
                .unwrap()
                .set_is_newly_heard(true);

            let table_items = song_list.db_table().select_all().unwrap();
            assert_eq!(table_items.len(), 2);

            for (id, song) in song_list.imp().list.borrow().iter() {
                assert_eq!(
                    table_items.get(id.as_str()).unwrap().is_newly_heard(),
                    song.is_newly_heard()
                );
            }
        }

        for (_, song) in song_list.db_table().select_all().unwrap() {
            assert!(song.is_newly_heard());
        }

        for (_, song) in song_list.imp().list.borrow().iter() {
            song.set_is_newly_heard(false);
        }

        for (_, song) in song_list.db_table().select_all().unwrap() {
            assert!(!song.is_newly_heard());
        }
    }

    #[test]
    fn load_from_db() {
        let db = Database::open_in_memory().unwrap();
        db.table::<Song>("songs")
            .unwrap()
            .insert_many(vec![
                ("Test-a", &new_test_song("a")),
                ("Test-b", &new_test_song("b")),
            ])
            .unwrap();

        let song_list = SongList::load_from_db(&db).unwrap();
        assert_eq!(song_list.n_items(), 2);

        assert_eq!(
            song_list.get(&SongId::new_for_test("a")).unwrap().id(),
            SongId::new_for_test("a")
        );
        assert_eq!(
            song_list.get(&SongId::new_for_test("b")).unwrap().id(),
            SongId::new_for_test("b")
        );

        assert_n_items_and_db_count_eq(&song_list, 2);
        assert_synced_to_db(&song_list);
    }

    #[test]
    fn append_and_remove() {
        let song_list = new_test_song_list();
        assert_n_items_and_db_count_eq(&song_list, 0);

        let song_1 = new_test_song("1");
        assert!(song_list.append(song_1.clone()));

        let song_2 = new_test_song("2");
        assert!(song_list.append(song_2.clone()));

        assert_n_items_and_db_count_eq(&song_list, 2);
        assert_synced_to_db(&song_list);

        assert_eq!(song_list.get(&song_1.id()), Some(song_1.clone()));
        assert_eq!(song_list.get(&song_2.id()), Some(song_2.clone()));

        let song_1_removed = song_list.remove(&song_1.id()).unwrap();
        assert_eq!(song_1, song_1_removed);
        assert_eq!(song_list.get(&song_1.id()), None);
        let song_2_removed = song_list.remove(&song_2.id()).unwrap();
        assert_eq!(song_2, song_2_removed);
        assert_eq!(song_list.get(&song_2.id()), None);

        assert_n_items_and_db_count_eq(&song_list, 0);

        // Ensure that the removed songs is not added back to the database
        song_1_removed.set_is_newly_heard(true);
        song_2_removed.set_is_newly_heard(true);
        assert_n_items_and_db_count_eq(&song_list, 0);
    }

    #[test]
    fn append_many() {
        let song_list = new_test_song_list();
        assert_n_items_and_db_count_eq(&song_list, 0);

        let songs = vec![new_test_song("1"), new_test_song("2")];
        assert_eq!(song_list.append_many(songs), 2);
        assert_n_items_and_db_count_eq(&song_list, 2);

        assert_synced_to_db(&song_list);

        let more_songs = vec![new_test_song("SameId"), new_test_song("SameId")];
        assert_eq!(song_list.append_many(more_songs), 1);
        assert_n_items_and_db_count_eq(&song_list, 3);
    }

    #[test]
    fn items_changed_append() {
        let song_list = new_test_song_list();

        song_list.connect_items_changed(|_, index, removed, added| {
            assert_eq!(index, 0);
            assert_eq!(removed, 0);
            assert_eq!(added, 1);
        });

        song_list.append(new_test_song("0"));
    }

    #[test]
    fn items_changed_append_index_1() {
        let song_list = new_test_song_list();
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
        let song_list = new_test_song_list();
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
        let song_list = new_test_song_list();
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
        let song_list = new_test_song_list();
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
        let song_list = new_test_song_list();
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
        let song_list = new_test_song_list();
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
        let song_list = new_test_song_list();

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
        let song_list = new_test_song_list();
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
        let song_list = new_test_song_list();

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
