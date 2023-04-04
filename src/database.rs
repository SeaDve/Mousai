use gtk::glib;
use once_cell::sync::Lazy;
use rusqlite_migration::{Migrations, M};

use std::fmt;

static MIGRATIONS: Lazy<Migrations<'static>> = Lazy::new(|| {
    Migrations::new(vec![M::up(
        "CREATE TABLE songs (
            id TEXT NOT NULL PRIMARY KEY, -- SongId
            title TEXT NOT NULL,
            artist TEXT NOT NULL,
            album TEXT NOT NULL,
            release_date TEXT,
            external_links TEXT NOT NULL, -- ExternalLinks
            album_art_link TEXT,
            playback_link TEXT,
            lyrics TEXT,
            last_heard TEXT, -- DateTime
            is_newly_heard BOOLEAN NOT NULL,

            is_in_history BOOLEAN NOT NULL
        );

        CREATE TABLE saved_recordings (
            id TEXT NOT NULL PRIMARY KEY,
            bytes BLOB NOT NULL,
            recorded_time TEXT NOT NULL, -- DateTime

            recognize_result_ok TEXT, -- SongId
            recognize_result_err TEXT -- RecognizeError
        );
        ",
    )])
});

pub fn new_connection() -> rusqlite::Connection {
    let path = glib::home_dir().join("data.db");
    let mut conn = rusqlite::Connection::open(path).unwrap();

    conn.pragma_update(None, "journal_mode", "WAL").unwrap();
    conn.pragma_update(None, "synchronous", "normal").unwrap();
    conn.pragma_update(None, "temp_store", "memory").unwrap();
    conn.pragma_update(None, "mmap_size", "30000000000")
        .unwrap();

    MIGRATIONS.to_latest(&mut conn).unwrap();

    conn
}

#[cfg(test)]
pub fn new_test_connection() -> rusqlite::Connection {
    let mut conn = rusqlite::Connection::open_in_memory().unwrap();

    MIGRATIONS.to_latest(&mut conn).unwrap();

    conn
}

pub struct ParamPlaceholders {
    n_placeholders: usize,
}

impl ParamPlaceholders {
    pub fn new(n_placeholders: usize) -> Self {
        Self { n_placeholders }
    }
}

impl fmt::Display for ParamPlaceholders {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for i in 0..self.n_placeholders {
            f.write_str("?")?;
            if i != self.n_placeholders - 1 {
                f.write_str(",")?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migrations() {
        assert!(MIGRATIONS.validate().is_ok());
    }

    #[test]
    fn param_placeholders() {
        assert_eq!(ParamPlaceholders::new(0).to_string(), "");
        assert_eq!(ParamPlaceholders::new(1).to_string(), "?");
        assert_eq!(ParamPlaceholders::new(5).to_string(), "?,?,?,?,?");
    }
}
