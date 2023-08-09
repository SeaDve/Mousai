use anyhow::{Context, Result};
use gtk::{
    gio,
    glib::{self, clone},
    prelude::*,
    subclass::prelude::*,
};
use heed::types::SerdeBincode;
use indexmap::IndexMap;

use std::{
    cell::{OnceCell, RefCell},
    collections::{BTreeSet, HashMap, HashSet},
    time::Instant,
};

use super::{Song, Uid, UidCodec};
use crate::{
    database::{EnvExt, SONG_LIST_DB_NAME},
    utils,
};

const SONG_NOTIFY_HANDLER_ID_KEY: &str = "mousai-song-notify-handler-id";

type SongDatabase = heed::Database<UidCodec, SerdeBincode<Song>>;

mod imp {
    use super::*;

    #[derive(Default)]
    pub struct SongList {
        pub(super) list: RefCell<IndexMap<Uid, Song>>,

        pub(super) db: OnceCell<(heed::Env, SongDatabase)>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for SongList {
        const NAME: &'static str = "MsaiSongList";
        type Type = super::SongList;
        type Interfaces = (gio::ListModel,);
    }

    impl ObjectImpl for SongList {}

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
        let db_load_start_time = Instant::now();

        let (db, songs) = env.with_write_txn(|wtxn| {
            let db: SongDatabase = env
                .create_database(wtxn, Some(SONG_LIST_DB_NAME))
                .context("Failed to create songs db")?;
            let songs = db
                .iter(wtxn)
                .context("Failed to iter songs from db")?
                .collect::<Result<IndexMap<_, _>, _>>()
                .context("Failed to collect songs from db")?;
            Ok((db, songs))
        })?;

        tracing::debug!(
            "Loaded {} songs in {:?}",
            songs.len(),
            db_load_start_time.elapsed()
        );
        for (id, song) in &songs {
            debug_assert_eq!(id, song.id_ref(), "id must be equal to song.id()");
        }

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

    /// If an equivalent song already exists in the list, it returns false and updates
    /// the corresponding value with `song`. Otherwise, it appends `song` at the
    /// end and returns true.
    ///
    /// The equivalence of the song depends on its [`Uid`].
    pub fn insert(&self, song: Song) -> Result<bool> {
        let (env, db) = self.db();
        env.with_write_txn(|wtxn| {
            db.put(wtxn, song.id_ref(), &song)
                .context("Failed to put song to db")?;
            Ok(())
        })?;

        self.bind_song_to_db(&song);
        let (position, prev_value) = self.imp().list.borrow_mut().insert_full(song.id(), song);

        if let Some(prev_value) = prev_value {
            unbind_song_from_db(&prev_value);
            self.items_changed(position as u32, 1, 1);
            Ok(false)
        } else {
            self.items_changed(position as u32, 0, 1);
            Ok(true)
        }
    }

    /// If an equivalent song already exists in the list, it updates the corresponding value
    /// with the given song. Otherwise, it appends the given song at the end.
    ///
    /// This returns the number of songs that were appended at the end, i.e., not updated.
    ///
    /// This is more efficient than [`SongList::insert`] since it emits `items-changed`
    /// only once for all appended songs.
    pub fn insert_many(&self, songs: Vec<Song>) -> Result<u32> {
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
                let (index, prev_value) = list.insert_full(song.id(), song);

                if let Some(prev_value) = prev_value {
                    unbind_song_from_db(&prev_value);
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

    pub fn remove_many(&self, song_ids: &[&Uid]) -> Result<Vec<Song>> {
        let imp = self.imp();

        let (env, db) = self.db();
        env.with_write_txn(|wtxn| {
            for song_id in song_ids {
                db.delete(wtxn, song_id)
                    .context("Failed to delete song from db")?;
            }
            Ok(())
        })?;

        let to_remove_indices = {
            let list = imp.list.borrow();
            song_ids
                .iter()
                .filter_map(|&song_id| list.get_index_of(song_id))
                .collect::<BTreeSet<_>>()
        };

        let removed = {
            let mut ret = Vec::with_capacity(to_remove_indices.len());

            // Reverse the iterations so we don't shift the indices
            for &(first, count) in utils::consecutive_groups(&to_remove_indices).iter().rev() {
                {
                    let mut list = imp.list.borrow_mut();

                    for index in (first..first + count).rev() {
                        let (_, song) =
                            list.shift_remove_index(index).expect("index must be valid");
                        unbind_song_from_db(&song);
                        ret.push(song);
                    }
                }

                self.items_changed(first as u32, count as u32, 0);
            }

            debug_assert_eq!(ret.len(), to_remove_indices.len());

            ret
        };

        Ok(removed)
    }

    pub fn get(&self, song_id: &Uid) -> Option<Song> {
        self.imp().list.borrow().get(song_id).cloned()
    }

    pub fn contains(&self, song_id: &Uid) -> bool {
        self.imp().list.borrow().contains_key(song_id)
    }

    pub fn is_empty(&self) -> bool {
        self.n_items() == 0
    }

    fn db(&self) -> &(heed::Env, SongDatabase) {
        self.imp().db.get().unwrap()
    }

    fn bind_song_to_db(&self, song: &Song) {
        unsafe {
            let handler_id = song.connect_notify_local(
                None,
                clone!(@weak self as obj => move |song, pspec| {
                    tracing::debug!("Song property `{}` notified", pspec.name());

                    let (env, db) = obj.db();
                    if let Err(err) = env.with_write_txn(|wtxn| {
                        debug_assert!(
                            db.get(wtxn, song.id_ref()).unwrap().is_some(),
                            "song must exist in the db"
                        );

                        db.put(wtxn, song.id_ref(), song)
                            .context("Failed to put song to db")?;

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

fn unbind_song_from_db(song: &Song) {
    unsafe {
        let handler_id = song
            .steal_data::<glib::SignalHandlerId>(SONG_NOTIFY_HANDLER_ID_KEY)
            .unwrap();
        song.disconnect(handler_id);
    };
}

/// Migrate from the old memory list of Mousai v0.6.6 and earlier.
fn migrate_from_memory_list(song_list: &SongList) -> Result<()> {
    use crate::{core::DateTime, model::ExternalLinkKey, settings::Settings};

    let settings = Settings::default();
    let memory_list = settings.memory_list();

    if memory_list.is_empty() {
        return Ok(());
    }

    tracing::debug!("Migrating {} songs from memory list", memory_list.len());

    let start_time = Instant::now();

    // We don't store last heards before, but we can get it from the creation date
    // of the songs' album art files
    let last_heards = {
        let mut ret = HashMap::new();

        let path = glib::user_cache_dir().join("tmp");

        match gio::File::for_path(&path).enumerate_children(
            gio::FILE_ATTRIBUTE_TIME_CREATED,
            gio::FileQueryInfoFlags::NOFOLLOW_SYMLINKS,
            gio::Cancellable::NONE,
        ) {
            Ok(enumerator) => {
                for res in enumerator {
                    let file_info = match res {
                        Ok(info) => info,
                        Err(err) => {
                            tracing::warn!("Failed to get album art file info: {:?}", err);
                            continue;
                        }
                    };

                    let file_name = file_info.name();

                    if file_info.file_type() != gio::FileType::Regular {
                        tracing::debug!("Skipping non-regular file `{}`", file_name.display());
                        continue;
                    }

                    let Some(creation_date_time) = file_info.creation_date_time() else {
                        tracing::debug!("Skipping file `{}` without creation date", file_name.display());
                        continue;
                    };

                    let Some(file_stem) = file_name.file_stem() else {
                        tracing::debug!("Skipping file `{}` without file stem", file_name.display());
                        continue;
                    };

                    ret.insert(file_stem.to_string_lossy().to_string(), creation_date_time);
                }
            }
            Err(err) => {
                tracing::warn!(
                    "Failed to enumerate on path `{}`: {:?}",
                    path.display(),
                    err
                );
            }
        }

        ret
    };

    let songs = memory_list
        .into_iter()
        .map(|mut item| {
            let title = item.remove("title");
            let artist = item.remove("artist");
            let song_link = item.remove("song_link");
            let song_src = item.remove("song_src");

            let id = song_link.as_ref().map_or_else(Uid::generate, |song_link| {
                Uid::from_prefixed("AudD", song_link.trim_start_matches("https://lis.tn/"))
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

            if let (Some(title), Some(artist)) = (&title, &artist) {
                song_builder.external_link(
                    ExternalLinkKey::YoutubeSearchTerm,
                    format!("{} - {}", artist, title),
                );
            }

            if let Some(ref song_src) = song_src {
                song_builder.playback_link(song_src);
            }

            let song = song_builder.build();

            if let (Some(ref title), Some(ref artist)) = (title, artist) {
                // Some weird legacy stuff
                if let Some(creation_date_time) = last_heards.get(&format!("{}{}", title, artist)) {
                    song.set_last_heard(DateTime::from(
                        creation_date_time
                            .to_local()
                            .expect("date time must not go out of bounds"),
                    ));
                }
            }

            song
        })
        .collect::<Vec<_>>();
    song_list
        .insert_many(songs)
        .context("Failed to insert songs to song history")?;

    settings.set_memory_list(Vec::new());
    tracing::debug!(
        "Successfully migrated songs from memory list in {:?}",
        start_time.elapsed()
    );

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
        Song::builder(&Uid::from(id), id, id, id).build()
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
        db.put(&mut wtxn, &Uid::from("a"), &new_test_song("a"))
            .unwrap();
        db.put(&mut wtxn, &Uid::from("b"), &new_test_song("b"))
            .unwrap();
        wtxn.commit().unwrap();

        let song_list = SongList::load_from_env(env).unwrap();

        assert_eq!(
            song_list.get(&Uid::from("a")).unwrap().id_ref(),
            &Uid::from("a")
        );
        assert_eq!(
            song_list.get(&Uid::from("b")).unwrap().id_ref(),
            &Uid::from("b")
        );

        assert_n_items_and_db_count_eq(&song_list, 2);
        assert_synced_to_db(&song_list);
    }

    #[test]
    fn insert_and_remove() {
        let (env, _tempdir) = database::new_test_env();
        let song_list = SongList::load_from_env(env).unwrap();
        assert_n_items_and_db_count_eq(&song_list, 0);

        let song_1 = new_test_song("1");
        assert!(song_list.insert(song_1.clone()).unwrap());

        let song_2 = new_test_song("2");
        assert!(song_list.insert(song_2.clone()).unwrap());

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
            .insert_many(vec![song_1.clone(), song_2.clone()])
            .unwrap();
        assert_eq!(song_list.get(song_1.id_ref()), Some(song_1.clone()));
        assert_eq!(song_list.get(song_2.id_ref()), Some(song_2.clone()));
        assert_n_items_and_db_count_eq(&song_list, 2);

        let removed = song_list
            .remove_many(&[song_1.id_ref(), song_2.id_ref()])
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
    fn remove_many_reversed() {
        let (env, _tempdir) = database::new_test_env();
        let song_list = SongList::load_from_env(env).unwrap();
        assert_n_items_and_db_count_eq(&song_list, 0);

        let song_1 = new_test_song("1");
        let song_2 = new_test_song("2");

        song_list
            .insert_many(vec![song_1.clone(), song_2.clone()])
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
    fn insert_many() {
        let (env, _tempdir) = database::new_test_env();
        let song_list = SongList::load_from_env(env).unwrap();
        assert_n_items_and_db_count_eq(&song_list, 0);

        let songs = vec![new_test_song("1"), new_test_song("2")];
        assert_eq!(song_list.insert_many(songs).unwrap(), 2);
        assert_n_items_and_db_count_eq(&song_list, 2);

        assert_synced_to_db(&song_list);

        let more_songs = vec![new_test_song("SameId"), new_test_song("SameId")];
        assert_eq!(song_list.insert_many(more_songs).unwrap(), 1);
        assert_n_items_and_db_count_eq(&song_list, 3);
    }

    #[test]
    fn items_changed_insert() {
        let (env, _tempdir) = database::new_test_env();
        let song_list = SongList::load_from_env(env).unwrap();

        song_list.connect_items_changed(|_, index, removed, added| {
            assert_eq!(index, 0);
            assert_eq!(removed, 0);
            assert_eq!(added, 1);
        });

        song_list.insert(new_test_song("0")).unwrap();
    }

    #[test]
    fn items_changed_insert_index_1() {
        let (env, _tempdir) = database::new_test_env();
        let song_list = SongList::load_from_env(env).unwrap();
        song_list.insert(new_test_song("0")).unwrap();

        song_list.connect_items_changed(|_, index, removed, added| {
            assert_eq!(index, 1);
            assert_eq!(removed, 0);
            assert_eq!(added, 1);
        });

        song_list.insert(new_test_song("1")).unwrap();
    }

    #[test]
    fn items_changed_insert_equal() {
        let (env, _tempdir) = database::new_test_env();
        let song_list = SongList::load_from_env(env).unwrap();
        song_list.insert(new_test_song("0")).unwrap();

        let n_called = Rc::new(Cell::new(0));
        let n_called_clone = Rc::clone(&n_called);
        song_list.connect_items_changed(move |_, index, removed, added| {
            assert_eq!(index, 0);
            assert_eq!(removed, 1);
            assert_eq!(added, 1);
            n_called_clone.set(n_called_clone.get() + 1);
        });

        assert_eq!(n_called.get(), 0);
        song_list.insert(new_test_song("0")).unwrap();
        assert_eq!(n_called.get(), 1);
    }

    #[test]
    fn items_changed_insert_many() {
        let (env, _tempdir) = database::new_test_env();
        let song_list = SongList::load_from_env(env).unwrap();
        song_list.insert(new_test_song("0")).unwrap();

        song_list.connect_items_changed(|_, index, removed, added| {
            assert_eq!(index, 1);
            assert_eq!(removed, 0);
            assert_eq!(added, 2);
        });

        song_list
            .insert_many(vec![new_test_song("1"), new_test_song("2")])
            .unwrap();
    }

    #[test]
    fn items_changed_insert_many_with_duplicates() {
        let (env, _tempdir) = database::new_test_env();
        let song_list = SongList::load_from_env(env).unwrap();
        song_list.insert(new_test_song("0")).unwrap();

        let calls_output = Rc::new(RefCell::new(Vec::new()));
        let calls_output_clone = Rc::clone(&calls_output);
        song_list.connect_items_changed(move |_, index, removed, added| {
            calls_output_clone
                .borrow_mut()
                .push((index, removed, added));
        });

        assert_eq!(
            song_list
                .insert_many(vec![
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
    fn items_changed_insert_many_more_removed_than_n_items() {
        let (env, _tempdir) = database::new_test_env();
        let song_list = SongList::load_from_env(env).unwrap();
        song_list.insert(new_test_song("0")).unwrap();
        song_list.insert(new_test_song("1")).unwrap();

        let calls_output = Rc::new(RefCell::new(Vec::new()));
        let calls_output_clone = Rc::clone(&calls_output);
        song_list.connect_items_changed(move |_, index, removed, added| {
            calls_output_clone
                .borrow_mut()
                .push((index, removed, added));
        });

        assert_eq!(
            song_list
                .insert_many(vec![
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
        song_list.insert(new_test_song("0")).unwrap();

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
            song_list
                .remove_many(&[&Uid::from("0")])
                .unwrap()
                .pop()
                .unwrap()
                .id_ref(),
            &Uid::from("0")
        );
        assert_eq!(n_called.get(), 1);
    }

    #[test]
    fn items_changed_removed_none() {
        let (env, _tempdir) = database::new_test_env();
        let song_list = SongList::load_from_env(env).unwrap();
        song_list.insert(new_test_song("1")).unwrap();

        let n_called = Rc::new(Cell::new(0));
        let n_called_clone = Rc::clone(&n_called);
        song_list.connect_items_changed(move |_, index, removed, added| {
            assert_eq!(index, 0);
            assert_eq!(removed, 0);
            assert_eq!(added, 0);
            n_called_clone.set(n_called_clone.get() + 1);
        });

        assert_eq!(n_called.get(), 0);
        assert!(song_list
            .remove_many(&[&Uid::from("0")])
            .unwrap()
            .is_empty());
        assert_eq!(n_called.get(), 0);
    }

    #[test]
    fn items_changed_removed_many() {
        let (env, _tempdir) = database::new_test_env();
        let song_list = SongList::load_from_env(env).unwrap();

        let song_0 = new_test_song("0");
        let song_1 = new_test_song("1");
        song_list.insert_many(vec![song_0, song_1]).unwrap();

        let calls_output = Rc::new(RefCell::new(Vec::new()));
        let calls_output_clone = Rc::clone(&calls_output);
        song_list.connect_items_changed(move |_, index, removed, added| {
            calls_output_clone
                .borrow_mut()
                .push((index, removed, added));
        });

        assert_eq!(
            song_list
                .remove_many(&[&Uid::from("0"), &Uid::from("1")])
                .unwrap()
                .len(),
            2
        );
        assert_eq!(calls_output.take(), &[(0, 2, 0)]);
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
            .insert_many(vec![song_0, song_1, song_2, song_3, song_4])
            .unwrap();

        let calls_output = Rc::new(RefCell::new(Vec::new()));
        let calls_output_clone = Rc::clone(&calls_output);
        song_list.connect_items_changed(move |_, index, removed, added| {
            calls_output_clone
                .borrow_mut()
                .push((index, removed, added));
        });

        assert_eq!(
            song_list
                .remove_many(&[&Uid::from("1"), &Uid::from("3")])
                .unwrap()
                .len(),
            2
        );
        assert_eq!(calls_output.take(), &[(3, 1, 0), (1, 1, 0)]);
    }

    #[test]
    fn items_changed_removed_many_with_duplicates() {
        let (env, _tempdir) = database::new_test_env();
        let song_list = SongList::load_from_env(env).unwrap();

        let song_0 = new_test_song("0");
        let song_1 = new_test_song("1");
        song_list.insert_many(vec![song_0, song_1]).unwrap();

        let calls_output = Rc::new(RefCell::new(Vec::new()));
        let calls_output_clone = Rc::clone(&calls_output);
        song_list.connect_items_changed(move |_, index, removed, added| {
            calls_output_clone
                .borrow_mut()
                .push((index, removed, added));
        });

        assert_eq!(
            song_list
                .remove_many(&[&Uid::from("1"), &Uid::from("0"), &Uid::from("1"),])
                .unwrap()
                .len(),
            2
        );
        assert_eq!(calls_output.take(), &[(0, 2, 0)]);
    }

    #[test]
    fn items_changed_removed_many_reversed_order() {
        let (env, _tempdir) = database::new_test_env();
        let song_list = SongList::load_from_env(env).unwrap();

        let song_0 = new_test_song("0");
        let song_1 = new_test_song("1");
        song_list.insert_many(vec![song_0, song_1]).unwrap();

        let calls_output = Rc::new(RefCell::new(Vec::new()));
        let calls_output_clone = Rc::clone(&calls_output);
        song_list.connect_items_changed(move |_, index, removed, added| {
            calls_output_clone
                .borrow_mut()
                .push((index, removed, added));
        });

        assert_eq!(
            song_list
                .remove_many(&[&Uid::from("1"), &Uid::from("0")])
                .unwrap()
                .len(),
            2
        );
        assert_eq!(calls_output.take(), &[(0, 2, 0)]);
    }

    #[test]
    fn items_changed_removed_many_none() {
        let (env, _tempdir) = database::new_test_env();
        let song_list = SongList::load_from_env(env).unwrap();
        song_list
            .insert_many(vec![new_test_song("1"), new_test_song("2")])
            .unwrap();

        let n_called = Rc::new(Cell::new(0));
        let n_called_clone = Rc::clone(&n_called);
        song_list.connect_items_changed(move |_, _index, _removed, _added| {
            n_called_clone.set(n_called_clone.get() + 1);
        });

        assert!(song_list
            .remove_many(&[&Uid::from("0"), &Uid::from("3")])
            .unwrap()
            .is_empty(),);
        assert_eq!(n_called.get(), 0);
    }
}
