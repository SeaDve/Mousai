use gtk::glib;
use rusqlite::Connection;

thread_local! {
    static CONNECTION: Connection = Connection::open(glib::home_dir().join("data.db")).unwrap();
}

pub fn ensure() {
    CONNECTION.with(|conn| {
        conn.execute(
            "CREATE TABLE IF NOT EXISTS songs (
                id             CHAR    NOT NULL,
                title          VARCHAR NOT NULL,
                artist         VARCHAR NOT NULL,
                album          VARCHAR NOT NULL,
                release_date   VARCHAR,
                external_links VARCHAR, -- JSON
                album_art_link VARCHAR,
                playback_link  VARCHAR,
                lyrics         VARCHAR,
                last_heard     VARCHAR, -- DateTime
                is_newly_heard BOOLEAN NOT NULL,
                PRIMARY KEY(id)
            )",
            (),
        )
        .unwrap();
    });
}

pub mod song {
    use rusqlite::Result;

    use super::CONNECTION;
    use crate::{
        core::DateTime,
        debug_assert_or_log,
        model::{Song, SongId},
    };

    /// Inserts a song into the database if it doesn't exist based on the id,
    /// otherwise updates the existing song.
    pub fn insert_or_update(song: &Song) -> Result<()> {
        let changed = CONNECTION.with(|conn| {
            conn.execute(
                "INSERT INTO songs (
                    id,
                    title,
                    artist,
                    album,
                    release_date,
                    external_links,
                    album_art_link,
                    playback_link,
                    lyrics,
                    last_heard,
                    is_newly_heard
                )
                    VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
                    ON CONFLICT(id) DO UPDATE SET
                        title = ?2,
                        artist = ?3,
                        album = ?4,
                        release_date = ?5,
                        external_links = ?6,
                        album_art_link = ?7,
                        playback_link = ?8,
                        lyrics = ?9,
                        last_heard = ?10,
                        is_newly_heard = ?11
                ",
                (
                    song.id(),
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
                ),
            )
        })?;
        debug_assert_or_log!(changed == 1);
        Ok(())
    }

    pub fn read_all() -> Result<Vec<Song>> {
        CONNECTION.with(|conn| {
            let mut stmt = conn.prepare("SELECT * FROM songs")?;
            let res = stmt
                .query_map([], |row| Song::try_from(row))?
                .collect::<rusqlite::Result<Vec<Song>>>();
            res
        })
    }

    pub fn update_is_newly_heard(id: &SongId, is_newly_heard: bool) -> Result<()> {
        let changed = CONNECTION.with(|conn| {
            conn.execute(
                "UPDATE songs SET is_newly_heard = ?1 WHERE id = ?2",
                (is_newly_heard, id),
            )
        })?;
        debug_assert_or_log!(changed == 1);
        Ok(())
    }

    pub fn update_last_heard(id: &SongId, last_heard: DateTime) -> Result<()> {
        let changed = CONNECTION.with(|conn| {
            conn.execute(
                "UPDATE songs SET last_heard = ?1 WHERE id = ?2",
                (last_heard, id),
            )
        })?;
        debug_assert_or_log!(changed == 1);
        Ok(())
    }

    pub fn delete(id: &SongId) -> Result<()> {
        let changed =
            CONNECTION.with(|conn| conn.execute("DELETE FROM songs WHERE id = ?1", (id,)))?;
        debug_assert_or_log!(changed == 1);
        Ok(())
    }
}
