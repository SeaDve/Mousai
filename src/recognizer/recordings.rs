use anyhow::Result;
use gtk::{
    gio,
    glib::{self, clone},
    prelude::*,
    subclass::prelude::*,
};
use indexmap::IndexMap;
use once_cell::unsync::OnceCell;

use std::{cell::RefCell, rc::Rc};

use super::Recording;
use crate::{
    core::{Bytes, DateTime},
    database::ParamPlaceholders,
    debug_assert_eq_or_log, debug_assert_or_log,
};

const RECORDING_RECOGNIZE_RESULT_NOTIFY_HANDLER_ID_KEY: &str =
    "mousai-recording-recognize-result-notify-handler-id";

mod imp {
    use super::*;

    #[derive(Debug, Default)]
    pub struct Recordings {
        pub(super) list: RefCell<IndexMap<String, Recording>>,

        pub(super) db_conn: OnceCell<Rc<rusqlite::Connection>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Recordings {
        const NAME: &'static str = "MsaiRecordings";
        type Type = super::Recordings;
        type Interfaces = (gio::ListModel,);
    }

    impl ObjectImpl for Recordings {}

    impl ListModelImpl for Recordings {
        fn item_type(&self) -> glib::Type {
            Recording::static_type()
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
    pub struct Recordings(ObjectSubclass<imp::Recordings>)
        @implements gio::ListModel;
}

impl Recordings {
    /// Load from the `saved_recordings` table in the database
    pub fn load_from_db(conn: Rc<rusqlite::Connection>) -> Result<Self> {
        let now = std::time::Instant::now();
        let recordings = {
            let mut stmt = conn.prepare_cached(&format!(
                "SELECT id, bytes, recorded_time, recognize_result_ok, recognize_result_err FROM saved_recordings",
            ))?;
            let res = stmt.query_map([], |row| {
                let id = row.get::<_, String>(0)?;
                let recording = Recording::from_raw_parts(
                    id.clone(),
                    row.get::<_, Bytes>(1)?,
                    row.get::<_, DateTime>(2)?,
                    recognize_result,
                );
                Ok((id, recording))
            })?;
            res.collect::<rusqlite::Result<IndexMap<_, _>>>()?
        };
        tracing::debug!(
            "Loaded {} saved recordings in {:?}",
            recordings.len(),
            now.elapsed()
        );

        let this = glib::Object::new::<Self>();

        for (_, recording) in recordings.iter() {
            this.bind_recording_to_items_changed_and_db(recording);
        }

        let imp = this.imp();
        imp.list.replace(recordings);
        imp.db_conn.set(conn).unwrap();

        Ok(this)
    }

    pub fn insert(&self, recording: Recording) {
        let conn = self.db_conn();
        let txn = conn.unchecked_transaction().unwrap();
        {
            let mut stmt = conn
            .prepare_cached(&format!(
                "INSERT INTO saved_recordings (id, bytes, recorded_time, recognize_result_ok, recognize_result_err) VALUES ({})",
                ParamPlaceholders::new(5)
            ))
            .unwrap();
            let n_changed = stmt
                .execute((
                    recording.id(),
                    recording.bytes(),
                    recording.recorded_time(),
                    recording
                        .recognize_result()
                        .and_then(|r| r.0.ok().map(|s| s.id_ref())),
                    recording.recognize_result().and_then(|r| r.0.err()),
                ))
                .unwrap();
            debug_assert_eq_or_log!(n_changed, 1);

            if let Some(song) = recording.recognize_result().and_then(|r| r.0.ok()) {
                let mut stmt = conn
                    .prepare_cached(&format!(
                        "INSERT INTO songs (id, title, artist, album, release_date, external_links, album_art_link, playback_link, lyrics, last_heard, is_newly_heard, is_in_history) VALUES ({})",
                        ParamPlaceholders::new(12)
                    ))
                    .unwrap();
                let n_changed = stmt
                    .execute((
                        song.id_ref(),
                        song.title(),
                        song.artist(),
                        song.album(),
                        song.release_date(),
                        song.external_links(),
                        song.album_art_link(),
                        song.playback_link(),
                        song.lyrics(),
                        song.last_heard(),
                        song.is_newly_heard(),
                        false,
                    ))
                    .unwrap();
                debug_assert_eq_or_log!(n_changed, 1);
            }
        }
        txn.commit().unwrap();

        self.bind_recording_to_items_changed_and_db(&recording);

        let (position, last_value) = self
            .imp()
            .list
            .borrow_mut()
            .insert_full(recording.id(), recording);
        debug_assert_or_log!(last_value.is_none());

        self.items_changed(position as u32, 0, 1);
    }

    pub fn peek_filtered(&self, filter_func: impl Fn(&Recording) -> bool) -> Vec<Recording> {
        let imp = self.imp();

        imp.list
            .borrow()
            .iter()
            .filter(|(_, recording)| filter_func(recording))
            .map(|(_, recording)| recording.clone())
            .collect()
    }

    pub fn take_filtered(&self, filter_func: impl Fn(&Recording) -> bool) -> Vec<Recording> {
        let imp = self.imp();

        let mut to_take_ids = Vec::new();
        for (id, recording) in imp.list.borrow().iter() {
            if filter_func(recording) {
                to_take_ids.push(id.to_string());
            }
        }

        let conn = self.db_conn();
        let txn = conn.unchecked_transaction().unwrap();
        {
            let mut stmt = conn
                .prepare_cached("DELETE FROM saved_recordings WHERE id = ?")
                .unwrap();
            for id in &to_take_ids {
                let n_changed = stmt.execute((id,)).unwrap();
                debug_assert_eq_or_log!(n_changed, 1);

                let recording = imp.list.borrow().get(id).unwrap();
                if let Some(song) = recording.recognize_result().and_then(|r| r.0.ok()) {
                    let mut stmt = conn
                        .prepare_cached("DELETE FROM songs WHERE id = ?")
                        .unwrap();
                    let n_changed = stmt.execute((song.id_ref(),)).unwrap();
                    debug_assert_eq_or_log!(n_changed, 1);
                }
            }
        }
        txn.commit().unwrap();

        let mut taken = Vec::new();
        for id in &to_take_ids {
            let (index, _, recording) = imp
                .list
                .borrow_mut()
                .shift_remove_full(id.as_str())
                .expect("id exists");
            unbind_recording_to_items_changed_and_db(&recording);
            self.items_changed(index as u32, 1, 0); // TODO Optimize this
            taken.push(recording);
        }

        taken
    }

    pub fn is_empty(&self) -> bool {
        self.n_items() == 0
    }

    fn db_conn(&self) -> &rusqlite::Connection {
        self.imp().db_conn.get().unwrap()
    }

    fn bind_recording_to_items_changed_and_db(&self, recording: &Recording) {
        unsafe {
            let handler_id = recording.connect_recognize_result_notify(
                clone!(@weak self as obj => move |recording| {
                    obj.recording_recognize_result_notify(recording);
                }),
            );
            recording.set_data(RECORDING_RECOGNIZE_RESULT_NOTIFY_HANDLER_ID_KEY, handler_id);
        }
    }

    fn recording_recognize_result_notify(&self, recording: &Recording) {
        let conn = self.db_conn();
        let txn = conn.unchecked_transaction().unwrap();
        {
            let mut stmt = conn
            .prepare_cached("UPDATE saved_recordings SET recognize_result_ok = ?, recognize_result_err = ? WHERE id = ?")
            .unwrap();
            let n_changed = stmt
                .execute((
                    recording
                        .recognize_result()
                        .and_then(|r| r.0.ok().map(|s| s.id_ref())),
                    recording.recognize_result().and_then(|r| r.0.err()),
                    recording.id(),
                ))
                .unwrap();
            debug_assert_eq_or_log!(n_changed, 1);

            if let Some(song) = recording.recognize_result().and_then(|r| r.0.ok()) {
                let mut stmt = conn
                    .prepare_cached(&format!(
                        "INSERT INTO songs (id, title, artist, album, release_date, external_links, album_art_link, playback_link, lyrics, last_heard, is_newly_heard, is_in_history) VALUES ({})",
                        ParamPlaceholders::new(12)
                    ))
                    .unwrap();
                let n_changed = stmt
                    .execute((
                        song.id_ref(),
                        song.title(),
                        song.artist(),
                        song.album(),
                        song.release_date(),
                        song.external_links(),
                        song.album_art_link(),
                        song.playback_link(),
                        song.lyrics(),
                        song.last_heard(),
                        song.is_newly_heard(),
                        false,
                    ))
                    .unwrap();
                debug_assert_eq_or_log!(n_changed, 1);
            } else {
                let mut stmt = conn
                    .prepare_cached("DELETE FROM songs WHERE id = ?")
                    .unwrap();
                let n_changed = stmt.execute((recording.id(),)).unwrap();
                debug_assert_eq_or_log!(n_changed, 1);
            }
        }
        txn.commit().unwrap();

        let index = self
            .imp()
            .list
            .borrow()
            .get_index_of(&recording.id())
            .unwrap();
        self.items_changed(index as u32, 1, 1);
    }
}

fn unbind_recording_to_items_changed_and_db(recording: &Recording) {
    unsafe {
        let handler_id = recording
            .steal_data::<glib::SignalHandlerId>(RECORDING_RECOGNIZE_RESULT_NOTIFY_HANDLER_ID_KEY)
            .unwrap();
        recording.disconnect(handler_id);
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::rc::Rc;

    use crate::{
        core::DateTime,
        database,
        model::SongId,
        recognizer::{
            recording::{create_recognize_result, BoxedRecognizeResult},
            RecognizeError,
        },
        utils,
    };

    fn new_test_recording(bytes: &'static [u8]) -> Recording {
        Recording::new(
            &utils::generate_unique_id(),
            &bytes.into(),
            &DateTime::now_local(),
        )
    }

    fn assert_n_items_and_db_count_eq(recordings: &Recordings, n: usize) {
        assert_eq!(recordings.n_items(), n as u32);

        let conn = recordings.db_conn();
        let count = conn
            .query_row("SELECT COUNT(id) FROM saved_recordings", (), |row| {
                row.get::<_, u64>(0)
            })
            .unwrap();
        assert_eq!(count, n as u64);
    }

    #[track_caller]
    fn assert_equal_recognize_result_song_id(a: &Recording, b: &Recording) {
        match (a.recognize_result(), b.recognize_result()) {
            (Some(result_a), Some(result_b)) => {
                assert_eq!(result_a.0.as_ref().unwrap(), result_b.0.as_ref().unwrap());
            }
            (a, b) => assert_eq!(a, b),
        }
    }

    /// Must have exactly 2 recordings
    fn assert_synced_to_db(recordings: &Recordings) {
        assert_n_items_and_db_count_eq(recordings, 2);

        let conn = recordings.db_conn();
        let mut all_recognize_result_stmt = conn
            .prepare_cached(
                "SELECT recognize_result_ok, recognize_result_err FROM saved_recordings",
            )
            .unwrap();
        let mut get_recording_stmt = conn
            .prepare_cached(&format!(
                "SELECT {} FROM saved_recordings WHERE id = ?",
                Recording::param_fields()
            ))
            .unwrap();

        // Test if the items are synced to the database
        for recognize_result in all_recognize_result_stmt
            .query_map((), |row| {
                Ok(create_recognize_result(
                    row.get::<_, Option<SongId>>(0)?,
                    row.get::<_, Option<RecognizeError>>(1)?,
                ))
            })
            .unwrap()
        {
            assert!(recognize_result.unwrap().unwrap().is_none());
        }
        for (id, recording) in recordings.imp().list.borrow().iter() {
            assert_equal_recognize_result_song_id(
                &get_recording_stmt
                    .query_row((id,), |row| Recording::try_from(row))
                    .unwrap(),
                recording,
            );
        }

        recordings
            .item(0)
            .and_downcast::<Recording>()
            .unwrap()
            .set_recognize_result(Some(BoxedRecognizeResult(Ok(SongId::for_test("a")))));
        assert_n_items_and_db_count_eq(recordings, 2);

        // Test if the items are synced to the database even
        // after the recording is modified
        for (id, recording) in recordings.imp().list.borrow().iter() {
            assert_equal_recognize_result_song_id(
                &get_recording_stmt
                    .query_row((id,), |row| Recording::try_from(row))
                    .unwrap(),
                recording,
            );
        }

        recordings
            .item(1)
            .and_downcast::<Recording>()
            .unwrap()
            .set_recognize_result(Some(BoxedRecognizeResult(Ok(SongId::for_test("b")))));
        assert_n_items_and_db_count_eq(recordings, 2);

        for recognize_result in all_recognize_result_stmt
            .query_map((), |row| {
                Ok(create_recognize_result(
                    row.get::<_, Option<SongId>>(0)?,
                    row.get::<_, Option<RecognizeError>>(1)?,
                ))
            })
            .unwrap()
        {
            assert!(recognize_result.unwrap().unwrap().is_some());
        }
        for (id, recording) in recordings.imp().list.borrow().iter() {
            assert_equal_recognize_result_song_id(
                &get_recording_stmt
                    .query_row((id,), |row| Recording::try_from(row))
                    .unwrap(),
                recording,
            );
        }

        for (_, recording) in recordings.imp().list.borrow().iter() {
            recording.set_recognize_result(None::<BoxedRecognizeResult>);
        }

        for recognize_result in all_recognize_result_stmt
            .query_map((), |row| {
                Ok(create_recognize_result(
                    row.get::<_, Option<SongId>>(0)?,
                    row.get::<_, Option<RecognizeError>>(1)?,
                ))
            })
            .unwrap()
        {
            assert!(recognize_result.unwrap().unwrap().is_none());
        }
    }

    #[test]
    fn load_from_db() {
        let conn = database::new_test_connection();
        {
            let mut insert_stmt = conn
                .prepare_cached(&format!(
                    "INSERT INTO saved_recordings ({}) VALUES ({})",
                    Recording::param_fields(),
                    Recording::param_placeholders()
                ))
                .unwrap();
            assert_eq!(
                insert_stmt
                    .execute(new_test_recording(b"A").param_values())
                    .unwrap(),
                1
            );
            assert_eq!(
                insert_stmt
                    .execute(new_test_recording(b"B").param_values())
                    .unwrap(),
                1
            );
        }
        let recordings = Recordings::load_from_db(Rc::new(conn)).unwrap();

        let items = recordings.peek_filtered(|_| true);
        assert!(items.iter().any(|i| i.bytes().as_ref() == b"A"));
        assert!(items.iter().any(|i| i.bytes().as_ref() == b"B"));

        assert_n_items_and_db_count_eq(&recordings, 2);
        assert_synced_to_db(&recordings);
    }

    #[test]
    fn insert() {
        let conn = database::new_test_connection();
        let recordings = Recordings::load_from_db(Rc::new(conn)).unwrap();
        assert_n_items_and_db_count_eq(&recordings, 0);

        recordings.insert(new_test_recording(b"a"));
        assert_n_items_and_db_count_eq(&recordings, 1);
        assert_eq!(
            recordings
                .item(0)
                .and_downcast::<Recording>()
                .unwrap()
                .bytes()
                .as_ref(),
            b"a"
        );

        recordings.insert(new_test_recording(b"b"));
        assert_n_items_and_db_count_eq(&recordings, 2);
        assert_eq!(
            recordings
                .item(1)
                .and_downcast::<Recording>()
                .unwrap()
                .bytes()
                .as_ref(),
            b"b"
        );

        assert_synced_to_db(&recordings);
    }

    #[test]
    fn insert_items_changed() {
        let conn = database::new_test_connection();
        let recordings = Recordings::load_from_db(Rc::new(conn)).unwrap();

        recordings.connect_items_changed(|_, index, removed, added| {
            assert_eq!(index, 0);
            assert_eq!(removed, 0);
            assert_eq!(added, 1);
        });

        recordings.insert(new_test_recording(b"a"));
    }

    #[test]
    fn peek_filtered() {
        let conn = database::new_test_connection();
        let recordings = Recordings::load_from_db(Rc::new(conn)).unwrap();
        assert_n_items_and_db_count_eq(&recordings, 0);

        assert!(recordings.peek_filtered(|_| false).is_empty());
        assert!(recordings.peek_filtered(|_| true).is_empty());
        assert!(recordings
            .peek_filtered(|r| r.bytes().as_ref() == b"a")
            .is_empty());

        recordings.insert(new_test_recording(b"a"));
        assert!(recordings.peek_filtered(|_| false).is_empty());
        assert_eq!(recordings.peek_filtered(|_| true).len(), 1);
        assert_eq!(
            recordings
                .peek_filtered(|r| r.bytes().as_ref() == b"a")
                .len(),
            1,
        );
        assert_n_items_and_db_count_eq(&recordings, 1);

        recordings.insert(new_test_recording(b"b"));
        assert!(recordings.peek_filtered(|_| false).is_empty());
        assert_eq!(recordings.peek_filtered(|_| true).len(), 2);
        assert_eq!(
            recordings
                .peek_filtered(|r| r.bytes().as_ref() == b"a")
                .len(),
            1,
        );
        assert_n_items_and_db_count_eq(&recordings, 2);
    }

    #[test]
    fn peek_filtered_items_changed() {
        let conn = database::new_test_connection();
        let recordings = Recordings::load_from_db(Rc::new(conn)).unwrap();

        let a_handler_id = recordings.connect_items_changed(|_, _, _, _| {
            panic!("Recordings::items_changed should not be emitted when peek_filtered is called");
        });
        recordings.peek_filtered(|_| true);
        recordings.peek_filtered(|_| false);

        recordings.disconnect(a_handler_id);
        recordings.insert(new_test_recording(b"a"));

        recordings.connect_items_changed(|_, _, _, _| {
            panic!("Recordings::items_changed should not be emitted when peek_filtered is called");
        });
        recordings.peek_filtered(|_| true);
        recordings.peek_filtered(|_| false);
    }

    #[test]
    fn take_filtered() {
        let conn = database::new_test_connection();
        let recordings = Recordings::load_from_db(Rc::new(conn)).unwrap();

        assert_n_items_and_db_count_eq(&recordings, 0);
        assert!(recordings.take_filtered(|_| false).is_empty());
        assert!(recordings.take_filtered(|_| true).is_empty());
        assert!(recordings
            .take_filtered(|r| r.bytes().as_ref() == b"a")
            .is_empty());

        recordings.insert(new_test_recording(b"a"));
        assert_n_items_and_db_count_eq(&recordings, 1);
        assert!(recordings.take_filtered(|_| false).is_empty());
        assert_n_items_and_db_count_eq(&recordings, 1);
        assert_eq!(recordings.take_filtered(|_| true).len(), 1);
        assert_n_items_and_db_count_eq(&recordings, 0);

        recordings.insert(new_test_recording(b"a"));
        recordings.insert(new_test_recording(b"b"));
        assert_n_items_and_db_count_eq(&recordings, 2);
        assert!(recordings.take_filtered(|_| false).is_empty());
        assert_n_items_and_db_count_eq(&recordings, 2);

        let taken = recordings.take_filtered(|_| true);
        assert_eq!(taken.len(), 2);
        assert_n_items_and_db_count_eq(&recordings, 0);

        // Ensure that the removed recordings is not added back to the database
        for recording in taken {
            assert!(recording.recognize_result().is_none());
            recording.set_recognize_result(Some(BoxedRecognizeResult(Ok(SongId::for_test("a")))));
        }
        assert_n_items_and_db_count_eq(&recordings, 0);

        recordings.insert(new_test_recording(b"a"));
        recordings.insert(new_test_recording(b"b"));
        assert_eq!(
            recordings
                .take_filtered(|r| r.bytes().as_ref() == b"a")
                .len(),
            1,
        );
        assert_n_items_and_db_count_eq(&recordings, 1);
        assert_eq!(
            recordings
                .item(0)
                .and_downcast::<Recording>()
                .unwrap()
                .bytes()
                .as_ref(),
            b"b"
        );
    }

    #[test]
    fn take_filtered_items_changed() {
        let conn = database::new_test_connection();
        let recordings = Recordings::load_from_db(Rc::new(conn)).unwrap();
        recordings.insert(new_test_recording(b"a"));
        recordings.insert(new_test_recording(b"b"));

        let calls_output = Rc::new(RefCell::new(Vec::new()));

        let calls_output_clone = Rc::clone(&calls_output);
        let handler_id = recordings.connect_items_changed(move |_, index, removed, added| {
            calls_output_clone
                .borrow_mut()
                .push((index, removed, added));
        });

        recordings.take_filtered(|_| false);
        assert!(calls_output.take().is_empty());

        recordings.take_filtered(|_| true);
        assert_eq!(calls_output.take(), vec![(0, 1, 0), (0, 1, 0)]);

        recordings.block_signal(&handler_id);
        recordings.insert(new_test_recording(b"a"));
        recordings.insert(new_test_recording(b"b"));
        recordings.unblock_signal(&handler_id);

        recordings.take_filtered(|r| r.bytes().as_ref() == b"a");
        assert_eq!(calls_output.take(), vec![(0, 1, 0)]);
    }

    #[test]
    fn recording_notify_items_changed() {
        let conn = database::new_test_connection();
        let recordings = Recordings::load_from_db(Rc::new(conn)).unwrap();

        let recording_a = new_test_recording(b"a");
        recordings.insert(recording_a.clone());
        let recording_b = new_test_recording(b"b");
        recordings.insert(recording_b.clone());

        let calls_output = Rc::new(RefCell::new(Vec::new()));

        let calls_output_clone = Rc::clone(&calls_output);
        recordings.connect_items_changed(move |_, index, removed, added| {
            calls_output_clone
                .borrow_mut()
                .push((index, removed, added));
        });

        recording_a.set_recognize_result(Some(BoxedRecognizeResult(Ok(SongId::for_test("a")))));
        assert_eq!(calls_output.take(), vec![(0, 1, 1)]);

        recording_b.set_recognize_result(Some(BoxedRecognizeResult(Ok(SongId::for_test("a")))));
        assert_eq!(calls_output.take(), vec![(1, 1, 1)]);
    }

    #[test]
    fn is_empty() {
        let conn = database::new_test_connection();
        let recordings = Recordings::load_from_db(Rc::new(conn)).unwrap();

        assert!(recordings.is_empty());
        assert_n_items_and_db_count_eq(&recordings, 0);

        recordings.insert(new_test_recording(b"a"));
        assert!(!recordings.is_empty());
        assert_n_items_and_db_count_eq(&recordings, 1);
    }
}
