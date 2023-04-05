use anyhow::{Context, Result};
use gtk::{
    gio,
    glib::{self, clone, closure_local},
    prelude::*,
    subclass::prelude::*,
};
use heed::types::SerdeBincode;
use indexmap::IndexMap;
use once_cell::unsync::OnceCell;

use std::{cell::RefCell, collections::HashSet, time::Instant};

use super::{Song, SongId};
use crate::{
    database::{EnvExt, SONG_LIST_DB_NAME},
    debug_assert_or_log,
};

const SONG_NOTIFY_HANDLER_ID_KEY: &str = "mousai-song-notify-handler-id";

// FIXME Remove indirection of encoding SongId through SerdeBincode and use heed types directly
type SongDatabase = heed::Database<SerdeBincode<SongId>, SerdeBincode<Song>>;

#[derive(Clone, glib::Boxed)]
#[boxed_type(name = "MsaiBoxedSongVec")]
struct BoxedSongVec(Vec<Song>);

mod imp {
    use super::*;
    use glib::subclass::Signal;
    use once_cell::sync::Lazy;

    #[derive(Debug, Default)]
    pub struct SongList {
        pub(super) list: RefCell<IndexMap<SongId, Song>>,

        pub(super) db: OnceCell<(heed::Env, SongDatabase)>,
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
                    .param_types([BoxedSongVec::static_type()])
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
    pub fn load_from_env(env: heed::Env) -> Result<Self> {
        let now = Instant::now();

        let (db, songs) = env.with_write_txn(|wtxn| {
            let db = env
                .create_database(wtxn, Some(SONG_LIST_DB_NAME))
                .context("Failed to create songs db")?;
            let songs = db
                .iter(wtxn)
                .context("Failed to iter songs from db")?
                .collect::<Result<IndexMap<SongId, Song>, _>>()
                .context("Failed to collect songs from db")?;
            Ok((db, songs))
        })?;

        tracing::debug!("Loaded {} songs in {:?}", songs.len(), now.elapsed());
        debug_assert_or_log!(songs.iter().all(|(id, song)| id == song.id_ref()));

        let this = glib::Object::new::<Self>();

        for (_, song) in songs.iter() {
            this.bind_song_to_db(song);
        }

        let imp = this.imp();
        imp.list.replace(songs);
        imp.db.set((env, db)).unwrap();

        // TODO Remove in future releases
        migrate_from_memory_list(&this).context("Failed to migrate from memory list")?;

        Ok(this)
    }

    /// If an equivalent [`Song`] already exists in the list, it returns false and updates
    /// the original value in the list. Otherwise, it inserts the new [`Song`] at the end and
    /// returns true.
    ///
    /// The equivalence of the [`Song`] depends on its [`SongId`]
    pub fn append(&self, song: Song) -> Result<bool> {
        let (env, db) = self.db();
        env.with_write_txn(|wtxn| {
            db.put(wtxn, song.id_ref(), &song)
                .context("Failed to put song to db")?;
            Ok(())
        })?;

        self.bind_song_to_db(&song);
        let (position, last_value) = self.imp().list.borrow_mut().insert_full(song.id(), song);

        if let Some(last_value) = last_value {
            unbind_song_to_db(&last_value);
            self.items_changed(position as u32, 1, 1);
            Ok(false)
        } else {
            self.items_changed(position as u32, 0, 1);
            Ok(true)
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
    pub fn append_many(&self, songs: Vec<Song>) -> Result<u32> {
        let (env, db) = self.db();
        env.with_write_txn(|wtxn| {
            for song in &songs {
                db.put(wtxn, song.id_ref(), song)
                    .context("Failed to put song to db")?;
            }
            Ok(())
        })?;

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

        Ok(n_appended)
    }

    pub fn remove_many(&self, song_ids: &[&SongId]) -> Result<Vec<Song>> {
        let imp = self.imp();

        let (env, db) = self.db();
        env.with_write_txn(|wtxn| {
            for song_id in song_ids {
                db.delete(wtxn, song_id)
                    .context("Failed to delete song from db")?;
            }
            Ok(())
        })?;

        let mut taken = Vec::new();
        for song_id in song_ids {
            let removed = { imp.list.borrow_mut().shift_remove_full(*song_id) };
            if let Some((index, _, song)) = removed {
                unbind_song_to_db(&song);
                self.items_changed(index as u32, 1, 0); // TODO Optimize this
                taken.push(song);
            }
        }

        if !taken.is_empty() {
            self.emit_by_name::<()>("removed", &[&BoxedSongVec(taken.clone())]);
        }

        Ok(taken)
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
        F: Fn(&Self, &[Song]) + 'static,
    {
        self.connect_closure(
            "removed",
            true,
            closure_local!(|obj: &Self, boxed: BoxedSongVec| {
                f(obj, &boxed.0);
            }),
        )
    }

    fn db(&self) -> &(heed::Env, SongDatabase) {
        self.imp().db.get().unwrap()
    }

    fn bind_song_to_db(&self, song: &Song) {
        unsafe {
            let handler_id = song.connect_notify_local(
                None,
                clone!(@weak self as obj => move |song, _| {
                    let (env, db) = obj.db();
                    if let Err(err) = env.with_write_txn(|wtxn| {
                        db.put(wtxn, song.id_ref(), song).context("Failed to put song to db")?;
                        Ok(())
                    }) {
                        tracing::error!("Failed to update song in database: {:?}", err);
                    };
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

/// Migrate from the old memory list of Mousai v0.6.6 and earlier.
fn migrate_from_memory_list(song_list: &SongList) -> Result<()> {
    use crate::{model::ExternalLinkKey, settings::Settings};

    let settings = Settings::default();
    let memory_list = settings.memory_list();

    if memory_list.is_empty() {
        return Ok(());
    }

    tracing::debug!("Migrating {} songs from memory list", memory_list.len());

    let songs = memory_list
        .into_iter()
        .map(|mut item| {
            let title = item.remove("title");
            let artist = item.remove("artist");
            let song_link = item.remove("song_link");
            let song_src = item.remove("song_src");

            let id = song_link
                .as_ref()
                .map_or_else(SongId::generate_unique, |song_link| {
                    SongId::from("AudD", song_link.trim_start_matches("https://lis.tn/"))
                });

            let mut song_builder = Song::builder(
                &id,
                title.as_deref().unwrap_or_default(),
                artist.as_deref().unwrap_or_default(),
                "",
            );

            if let Some(song_link) = song_link {
                song_builder.external_link(ExternalLinkKey::AudDUrl, song_link);
            }

            if let (Some(ref title), Some(ref artist)) = (title, artist) {
                song_builder.external_link(
                    ExternalLinkKey::YoutubeSearchTerm,
                    format!("{} - {}", artist, title),
                );
            }

            if let Some(ref song_src) = song_src {
                song_builder.playback_link(song_src);
            }

            song_builder.build()
        })
        .collect::<Vec<_>>();
    song_list
        .append_many(songs)
        .context("Failed to append songs to song history")?;

    settings.set_memory_list(Vec::new());
    tracing::debug!("Successfully migrated songs from memory list");

    Ok(())
}

#[cfg(test)]
mod test {
    use super::*;

    use std::{
        cell::{Cell, RefCell},
        rc::Rc,
    };

    use crate::database;

    fn new_test_song(id: &str) -> Song {
        Song::builder(&SongId::for_test(id), id, id, id).build()
    }

    fn assert_n_items_and_db_count_eq(song_list: &SongList, n: usize) {
        assert_eq!(song_list.n_items(), n as u32);

        let (env, db) = song_list.db();
        let rtxn = env.read_txn().unwrap();
        assert_eq!(db.len(&rtxn).unwrap(), n as u64);
    }

    /// Must have exactly 2 songs
    fn assert_synced_to_db(song_list: &SongList) {
        assert_n_items_and_db_count_eq(song_list, 2);

        let (env, db) = song_list.db();

        // Test if the items are synced to the database
        let rtxn = env.read_txn().unwrap();
        for item in db.iter(&rtxn).unwrap() {
            let (_, song) = item.unwrap();
            assert!(!song.is_newly_heard());
        }
        for (id, song) in song_list.imp().list.borrow().iter() {
            assert_eq!(
                db.get(&rtxn, id).unwrap().unwrap().is_newly_heard(),
                song.is_newly_heard()
            );
        }
        drop(rtxn);

        song_list
            .item(0)
            .and_downcast::<Song>()
            .unwrap()
            .set_is_newly_heard(true);
        assert_n_items_and_db_count_eq(song_list, 2);

        // Test if the items are synced to the database even
        // after the song is modified\
        let rtxn = env.read_txn().unwrap();
        for (id, song) in song_list.imp().list.borrow().iter() {
            assert_eq!(
                db.get(&rtxn, id).unwrap().unwrap().is_newly_heard(),
                song.is_newly_heard()
            );
        }
        drop(rtxn);

        song_list
            .item(1)
            .and_downcast::<Song>()
            .unwrap()
            .set_is_newly_heard(true);
        assert_n_items_and_db_count_eq(song_list, 2);

        let rtxn = env.read_txn().unwrap();
        for item in db.iter(&rtxn).unwrap() {
            let (_, song) = item.unwrap();
            assert!(song.is_newly_heard());
        }
        for (id, song) in song_list.imp().list.borrow().iter() {
            assert_eq!(
                db.get(&rtxn, id).unwrap().unwrap().is_newly_heard(),
                song.is_newly_heard()
            );
        }
        drop(rtxn);

        for (_, song) in song_list.imp().list.borrow().iter() {
            song.set_is_newly_heard(false);
        }

        let rtxn = env.read_txn().unwrap();
        for item in db.iter(&rtxn).unwrap() {
            let (_, song) = item.unwrap();
            assert!(!song.is_newly_heard());
        }
    }

    #[test]
    fn load_from_db() {
        let (env, _tempdir) = database::new_test_env();
        let mut wtxn = env.write_txn().unwrap();
        let db: SongDatabase = env
            .create_database(&mut wtxn, Some(SONG_LIST_DB_NAME))
            .unwrap();
        db.put(&mut wtxn, &SongId::for_test("a"), &new_test_song("a"))
            .unwrap();
        db.put(&mut wtxn, &SongId::for_test("b"), &new_test_song("b"))
            .unwrap();
        wtxn.commit().unwrap();

        let song_list = SongList::load_from_env(env).unwrap();

        assert_eq!(
            song_list.get(&SongId::for_test("a")).unwrap().id_ref(),
            &SongId::for_test("a")
        );
        assert_eq!(
            song_list.get(&SongId::for_test("b")).unwrap().id_ref(),
            &SongId::for_test("b")
        );

        assert_n_items_and_db_count_eq(&song_list, 2);
        assert_synced_to_db(&song_list);
    }

    #[test]
    fn append_and_remove() {
        let (env, _tempdir) = database::new_test_env();
        let song_list = SongList::load_from_env(env).unwrap();
        assert_n_items_and_db_count_eq(&song_list, 0);

        let song_1 = new_test_song("1");
        assert!(song_list.append(song_1.clone()).unwrap());

        let song_2 = new_test_song("2");
        assert!(song_list.append(song_2.clone()).unwrap());

        assert_n_items_and_db_count_eq(&song_list, 2);
        assert_synced_to_db(&song_list);

        assert_eq!(song_list.get(song_1.id_ref()), Some(song_1.clone()));
        assert_eq!(song_list.get(song_2.id_ref()), Some(song_2.clone()));

        let song_1_removed = song_list
            .remove_many(&[song_1.id_ref()])
            .unwrap()
            .pop()
            .unwrap();
        assert_eq!(song_1, song_1_removed);
        assert_eq!(song_list.get(song_1.id_ref()), None);
        let song_2_removed = song_list
            .remove_many(&[song_2.id_ref()])
            .unwrap()
            .pop()
            .unwrap();
        assert_eq!(song_2, song_2_removed);
        assert_eq!(song_list.get(song_2.id_ref()), None);

        assert_n_items_and_db_count_eq(&song_list, 0);

        // Ensure that the removed songs is not added back to the database
        song_1.set_is_newly_heard(true);
        song_2.set_is_newly_heard(true);
        assert_n_items_and_db_count_eq(&song_list, 0);
    }

    #[test]
    fn remove_many() {
        let (env, _tempdir) = database::new_test_env();
        let song_list = SongList::load_from_env(env).unwrap();
        assert_n_items_and_db_count_eq(&song_list, 0);

        let song_1 = new_test_song("1");
        let song_2 = new_test_song("2");

        song_list
            .append_many(vec![song_1.clone(), song_2.clone()])
            .unwrap();
        assert_eq!(song_list.get(song_1.id_ref()), Some(song_1.clone()));
        assert_eq!(song_list.get(song_2.id_ref()), Some(song_2.clone()));
        assert_n_items_and_db_count_eq(&song_list, 2);

        let removed = song_list
            .remove_many(&[song_1.id_ref(), song_2.id_ref()])
            .unwrap();
        assert_eq!(removed, vec![song_1.clone(), song_2.clone()]);
        assert_eq!(song_list.get(song_1.id_ref()), None);
        assert_eq!(song_list.get(song_2.id_ref()), None);
        assert_n_items_and_db_count_eq(&song_list, 0);

        // Ensure that the removed songs is not added back to the database
        song_1.set_is_newly_heard(true);
        song_2.set_is_newly_heard(true);
        assert_n_items_and_db_count_eq(&song_list, 0);
    }

    #[test]
    fn remove_many_reversed() {
        let (env, _tempdir) = database::new_test_env();
        let song_list = SongList::load_from_env(env).unwrap();
        assert_n_items_and_db_count_eq(&song_list, 0);

        let song_1 = new_test_song("1");
        let song_2 = new_test_song("2");

        song_list
            .append_many(vec![song_1.clone(), song_2.clone()])
            .unwrap();
        assert_eq!(song_list.get(song_1.id_ref()), Some(song_1.clone()));
        assert_eq!(song_list.get(song_2.id_ref()), Some(song_2.clone()));
        assert_n_items_and_db_count_eq(&song_list, 2);

        let removed = song_list
            .remove_many(&[song_2.id_ref(), song_1.id_ref()])
            .unwrap();
        assert_eq!(removed, vec![song_2.clone(), song_1.clone()]);
        assert_eq!(song_list.get(song_1.id_ref()), None);
        assert_eq!(song_list.get(song_2.id_ref()), None);
        assert_n_items_and_db_count_eq(&song_list, 0);

        // Ensure that the removed songs is not added back to the database
        song_1.set_is_newly_heard(true);
        song_2.set_is_newly_heard(true);
        assert_n_items_and_db_count_eq(&song_list, 0);
    }

    #[test]
    fn append_many() {
        let (env, _tempdir) = database::new_test_env();
        let song_list = SongList::load_from_env(env).unwrap();
        assert_n_items_and_db_count_eq(&song_list, 0);

        let songs = vec![new_test_song("1"), new_test_song("2")];
        assert_eq!(song_list.append_many(songs).unwrap(), 2);
        assert_n_items_and_db_count_eq(&song_list, 2);

        assert_synced_to_db(&song_list);

        let more_songs = vec![new_test_song("SameId"), new_test_song("SameId")];
        assert_eq!(song_list.append_many(more_songs).unwrap(), 1);
        assert_n_items_and_db_count_eq(&song_list, 3);
    }

    #[test]
    fn items_changed_append() {
        let (env, _tempdir) = database::new_test_env();
        let song_list = SongList::load_from_env(env).unwrap();

        song_list.connect_items_changed(|_, index, removed, added| {
            assert_eq!(index, 0);
            assert_eq!(removed, 0);
            assert_eq!(added, 1);
        });

        song_list.append(new_test_song("0")).unwrap();
    }

    #[test]
    fn items_changed_append_index_1() {
        let (env, _tempdir) = database::new_test_env();
        let song_list = SongList::load_from_env(env).unwrap();
        song_list.append(new_test_song("0")).unwrap();

        song_list.connect_items_changed(|_, index, removed, added| {
            assert_eq!(index, 1);
            assert_eq!(removed, 0);
            assert_eq!(added, 1);
        });

        song_list.append(new_test_song("1")).unwrap();
    }

    #[test]
    fn items_changed_append_equal() {
        let (env, _tempdir) = database::new_test_env();
        let song_list = SongList::load_from_env(env).unwrap();
        song_list.append(new_test_song("0")).unwrap();

        let n_called = Rc::new(Cell::new(0));
        let n_called_clone = Rc::clone(&n_called);
        song_list.connect_items_changed(move |_, index, removed, added| {
            assert_eq!(index, 0);
            assert_eq!(removed, 1);
            assert_eq!(added, 1);
            n_called_clone.set(n_called_clone.get() + 1);
        });

        assert_eq!(n_called.get(), 0);
        song_list.append(new_test_song("0")).unwrap();
        assert_eq!(n_called.get(), 1);
    }

    #[test]
    fn items_changed_append_many() {
        let (env, _tempdir) = database::new_test_env();
        let song_list = SongList::load_from_env(env).unwrap();
        song_list.append(new_test_song("0")).unwrap();

        song_list.connect_items_changed(|_, index, removed, added| {
            assert_eq!(index, 1);
            assert_eq!(removed, 0);
            assert_eq!(added, 2);
        });

        song_list
            .append_many(vec![new_test_song("1"), new_test_song("2")])
            .unwrap();
    }

    #[test]
    fn items_changed_append_many_with_duplicates() {
        let (env, _tempdir) = database::new_test_env();
        let song_list = SongList::load_from_env(env).unwrap();
        song_list.append(new_test_song("0")).unwrap();

        let calls_output = Rc::new(RefCell::new(Vec::new()));
        let calls_output_clone = Rc::clone(&calls_output);
        song_list.connect_items_changed(move |_, index, removed, added| {
            calls_output_clone
                .borrow_mut()
                .push((index, removed, added));
        });

        assert_eq!(
            song_list
                .append_many(vec![
                    new_test_song("0"),
                    new_test_song("1"),
                    new_test_song("2"),
                    new_test_song("2"),
                ])
                .unwrap(),
            2
        );

        let calls_output = calls_output.borrow();
        assert_eq!(calls_output.len(), 2);
        assert_eq!(calls_output[0], (1, 0, 2));
        assert!(calls_output.contains(&(0, 1, 1)));
    }

    #[test]
    fn items_changed_append_many_more_removed_than_n_items() {
        let (env, _tempdir) = database::new_test_env();
        let song_list = SongList::load_from_env(env).unwrap();
        song_list.append(new_test_song("0")).unwrap();
        song_list.append(new_test_song("1")).unwrap();

        let calls_output = Rc::new(RefCell::new(Vec::new()));
        let calls_output_clone = Rc::clone(&calls_output);
        song_list.connect_items_changed(move |_, index, removed, added| {
            calls_output_clone
                .borrow_mut()
                .push((index, removed, added));
        });

        assert_eq!(
            song_list
                .append_many(vec![
                    new_test_song("0"),
                    new_test_song("0"),
                    new_test_song("0"),
                    new_test_song("1"),
                    new_test_song("2"),
                ])
                .unwrap(),
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
        let (env, _tempdir) = database::new_test_env();
        let song_list = SongList::load_from_env(env).unwrap();
        song_list.append(new_test_song("0")).unwrap();

        let ic_n_called = Rc::new(Cell::new(0));
        let ic_n_called_clone = Rc::clone(&ic_n_called);
        song_list.connect_items_changed(move |_, index, removed, added| {
            assert_eq!(index, 0);
            assert_eq!(removed, 1);
            assert_eq!(added, 0);
            ic_n_called_clone.set(ic_n_called_clone.get() + 1);
        });

        let r_n_called = Rc::new(Cell::new(0));
        let r_n_called_clone = Rc::clone(&r_n_called);
        song_list.connect_removed(move |_, songs| {
            assert_eq!(songs.len(), 1);
            assert_eq!(songs[0].id_ref(), &SongId::for_test("0"));
            r_n_called_clone.set(r_n_called_clone.get() + 1);
        });

        assert_eq!(ic_n_called.get(), 0);
        assert_eq!(r_n_called.get(), 0);
        assert_eq!(
            song_list
                .remove_many(&[&SongId::for_test("0")])
                .unwrap()
                .pop()
                .unwrap()
                .id_ref(),
            &SongId::for_test("0")
        );
        assert_eq!(ic_n_called.get(), 1);
        assert_eq!(r_n_called.get(), 1);
    }

    #[test]
    fn items_changed_removed_none() {
        let (env, _tempdir) = database::new_test_env();
        let song_list = SongList::load_from_env(env).unwrap();
        song_list.append(new_test_song("1")).unwrap();

        let ic_n_called = Rc::new(Cell::new(0));
        let ic_n_called_clone = Rc::clone(&ic_n_called);
        song_list.connect_items_changed(move |_, index, removed, added| {
            assert_eq!(index, 0);
            assert_eq!(removed, 0);
            assert_eq!(added, 0);
            ic_n_called_clone.set(ic_n_called_clone.get() + 1);
        });

        let r_n_called = Rc::new(Cell::new(0));
        let r_n_called_clone = Rc::clone(&r_n_called);
        song_list.connect_removed(move |_, _songs| {
            r_n_called_clone.set(r_n_called_clone.get() + 1);
        });

        assert_eq!(ic_n_called.get(), 0);
        assert_eq!(r_n_called.get(), 0);
        assert!(song_list
            .remove_many(&[&SongId::for_test("0")])
            .unwrap()
            .is_empty());
        assert_eq!(ic_n_called.get(), 0);
        assert_eq!(r_n_called.get(), 0);
    }

    #[test]
    fn items_changed_removed_many() {
        let (env, _tempdir) = database::new_test_env();
        let song_list = SongList::load_from_env(env).unwrap();

        let song_0 = new_test_song("0");
        let song_1 = new_test_song("1");
        song_list
            .append_many(vec![song_0.clone(), song_1.clone()])
            .unwrap();

        let ic_calls_output = Rc::new(RefCell::new(Vec::new()));
        let ic_calls_output_clone = Rc::clone(&ic_calls_output);
        song_list.connect_items_changed(move |_, index, removed, added| {
            ic_calls_output_clone
                .borrow_mut()
                .push((index, removed, added));
        });

        let r_calls_output = Rc::new(RefCell::new(Vec::new()));
        let r_calls_output_clone = Rc::clone(&r_calls_output);
        song_list.connect_removed(move |_, songs| {
            r_calls_output_clone.borrow_mut().push(songs.to_vec());
        });

        assert_eq!(
            song_list
                .remove_many(&[&SongId::for_test("0"), &SongId::for_test("1")])
                .unwrap()
                .len(),
            2
        );
        assert_eq!(ic_calls_output.take(), &[(0, 1, 0), (0, 1, 0)]);
        assert_eq!(r_calls_output.take(), vec![vec![song_0, song_1]]);
    }

    #[test]
    fn items_changed_removed_many_in_between() {
        let (env, _tempdir) = database::new_test_env();
        let song_list = SongList::load_from_env(env).unwrap();

        let song_0 = new_test_song("0");
        let song_1 = new_test_song("1");
        let song_2 = new_test_song("2");
        let song_3 = new_test_song("3");
        let song_4 = new_test_song("4");
        song_list
            .append_many(vec![song_0, song_1.clone(), song_2, song_3.clone(), song_4])
            .unwrap();

        let ic_calls_output = Rc::new(RefCell::new(Vec::new()));
        let ic_calls_output_clone = Rc::clone(&ic_calls_output);
        song_list.connect_items_changed(move |_, index, removed, added| {
            ic_calls_output_clone
                .borrow_mut()
                .push((index, removed, added));
        });

        let r_calls_output = Rc::new(RefCell::new(Vec::new()));
        let r_calls_output_clone = Rc::clone(&r_calls_output);
        song_list.connect_removed(move |_, songs| {
            r_calls_output_clone.borrow_mut().push(songs.to_vec());
        });

        assert_eq!(
            song_list
                .remove_many(&[&SongId::for_test("1"), &SongId::for_test("3")])
                .unwrap()
                .len(),
            2
        );
        assert_eq!(ic_calls_output.take(), &[(1, 1, 0), (2, 1, 0)]);
        assert_eq!(r_calls_output.take(), vec![vec![song_1, song_3]]);
    }

    #[test]
    fn items_changed_removed_many_with_duplicates() {
        let (env, _tempdir) = database::new_test_env();
        let song_list = SongList::load_from_env(env).unwrap();

        let song_0 = new_test_song("0");
        let song_1 = new_test_song("1");
        song_list
            .append_many(vec![song_0.clone(), song_1.clone()])
            .unwrap();

        let ic_calls_output = Rc::new(RefCell::new(Vec::new()));
        let ic_calls_output_clone = Rc::clone(&ic_calls_output);
        song_list.connect_items_changed(move |_, index, removed, added| {
            ic_calls_output_clone
                .borrow_mut()
                .push((index, removed, added));
        });

        let r_calls_output = Rc::new(RefCell::new(Vec::new()));
        let r_calls_output_clone = Rc::clone(&r_calls_output);
        song_list.connect_removed(move |_, songs| {
            r_calls_output_clone.borrow_mut().push(songs.to_vec());
        });

        assert_eq!(
            song_list
                .remove_many(&[
                    &SongId::for_test("1"),
                    &SongId::for_test("0"),
                    &SongId::for_test("1"),
                ])
                .unwrap()
                .len(),
            2
        );
        assert_eq!(ic_calls_output.take(), &[(1, 1, 0), (0, 1, 0)]);
        assert_eq!(r_calls_output.take(), vec![vec![song_1, song_0]]);
    }

    #[test]
    fn items_changed_removed_many_reversed_order() {
        let (env, _tempdir) = database::new_test_env();
        let song_list = SongList::load_from_env(env).unwrap();

        let song_0 = new_test_song("0");
        let song_1 = new_test_song("1");
        song_list
            .append_many(vec![song_0.clone(), song_1.clone()])
            .unwrap();

        let ic_calls_output = Rc::new(RefCell::new(Vec::new()));
        let ic_calls_output_clone = Rc::clone(&ic_calls_output);
        song_list.connect_items_changed(move |_, index, removed, added| {
            ic_calls_output_clone
                .borrow_mut()
                .push((index, removed, added));
        });

        let r_calls_output = Rc::new(RefCell::new(Vec::new()));
        let r_calls_output_clone = Rc::clone(&r_calls_output);
        song_list.connect_removed(move |_, songs| {
            r_calls_output_clone.borrow_mut().push(songs.to_vec());
        });

        assert_eq!(
            song_list
                .remove_many(&[&SongId::for_test("1"), &SongId::for_test("0")])
                .unwrap()
                .len(),
            2
        );
        assert_eq!(ic_calls_output.take(), &[(1, 1, 0), (0, 1, 0)]);
        assert_eq!(r_calls_output.take(), vec![vec![song_1, song_0]]);
    }

    #[test]
    fn items_changed_removed_many_none() {
        let (env, _tempdir) = database::new_test_env();
        let song_list = SongList::load_from_env(env).unwrap();
        song_list
            .append_many(vec![new_test_song("1"), new_test_song("2")])
            .unwrap();

        let ic_n_called = Rc::new(Cell::new(0));
        let ic_n_called_clone = Rc::clone(&ic_n_called);
        song_list.connect_items_changed(move |_, _index, _removed, _added| {
            ic_n_called_clone.set(ic_n_called_clone.get() + 1);
        });

        let r_n_called = Rc::new(Cell::new(0));
        let r_n_called_clone = Rc::clone(&r_n_called);
        song_list.connect_removed(move |_, _songs| {
            r_n_called_clone.set(r_n_called_clone.get() + 1);
        });

        assert!(song_list
            .remove_many(&[&SongId::for_test("0"), &SongId::for_test("3")])
            .unwrap()
            .is_empty(),);
        assert_eq!(ic_n_called.get(), 0);
        assert_eq!(r_n_called.get(), 0);
    }
}
