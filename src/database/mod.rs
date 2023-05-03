mod migrations;

use anyhow::{Context, Result};
use gtk::glib;

use std::{
    fs,
    time::{Duration, Instant},
};

pub use self::migrations::Migrations;

pub const USER_VERSION_KEY: &str = "user_version";

const N_NAMED_DBS: u32 = 2;
pub const SONG_LIST_DB_NAME: &str = "song_list";
pub const RECORDINGS_DB_NAME: &str = "saved_recordings";

/// Note: This must be only called once.
pub fn new_env() -> Result<heed::Env> {
    let path = glib::user_data_dir().join("mousai/db");
    fs::create_dir_all(&path)
        .with_context(|| format!("Failed to create db dir at {}", path.display()))?;
    let env = heed::EnvOpenOptions::new()
        .map_size(100 * 1024 * 1024) // 100 MiB
        .max_dbs(N_NAMED_DBS)
        .open(&path)
        .with_context(|| format!("Failed to open heed env at {}", path.display()))?;

    tracing::debug!(
        ?path,
        info = ?env.info(),
        real_disk_size = ?env.real_disk_size(),
        non_free_pages_size = ?env.non_free_pages_size(),
        "Opened db env"
    );

    Ok(env)
}

/// Create a new env for tests with 1 max named db and a
/// path to a temporary directory.
#[cfg(test)]
pub fn new_test_env() -> (heed::Env, tempfile::TempDir) {
    let tempdir = tempfile::tempdir().unwrap();
    let env = heed::EnvOpenOptions::new()
        .map_size(100 * 1024 * 1024) // 100 MiB
        .max_dbs(1)
        .open(&tempdir)
        .unwrap();
    (env, tempdir)
}

pub trait EnvExt {
    /// Run a func with a write txn and commit it.
    fn with_write_txn<T>(&self, func: impl FnOnce(&mut heed::RwTxn<'_>) -> Result<T>) -> Result<T>;
}

impl EnvExt for heed::Env {
    fn with_write_txn<T>(&self, func: impl FnOnce(&mut heed::RwTxn<'_>) -> Result<T>) -> Result<T> {
        let start_time = Instant::now();

        let mut wtxn = self.write_txn().context("Failed to create write txn")?;
        let ret = func(&mut wtxn)?;
        wtxn.commit().context("Failed to commit write txn")?;

        // There are 16.67 ms in a 60 Hz frame, so warn if the write txn
        // takes longer than that.
        if start_time.elapsed() > Duration::from_millis(15) {
            tracing::warn!("Database write txn took {:?}", start_time.elapsed());
        } else {
            tracing::trace!("Database write txn took {:?}", start_time.elapsed());
        }

        Ok(ret)
    }
}
