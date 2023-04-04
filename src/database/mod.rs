mod migrations;

use anyhow::{Context, Result};
use gtk::glib;

use std::fs;

pub use self::migrations::Migrations;

const N_NAMED_DBS: u32 = 2;

pub const SONG_LIST_DB_NAME: &str = "song_list";
pub const RECORDINGS_DB_NAME: &str = "saved_recordings";
pub const USER_VERSION_KEY: &str = "user_version";

/// Note: This must be only called once.
pub fn new_env() -> Result<heed::Env> {
    let path = glib::user_data_dir().join("mousai/db");
    fs::create_dir_all(&path)?;
    let env = unsafe {
        heed::EnvOpenOptions::new()
            .map_size(100 * 1024 * 1024) // 100 MiB
            .max_dbs(N_NAMED_DBS)
            .flag(heed::Flags::MdbWriteMap)
            .flag(heed::Flags::MdbMapAsync)
            .open(&path)
            .with_context(|| format!("Failed to open heed env at {}", path.display()))?
    };

    tracing::debug!(
        ?path,
        info = ?env.info(),
        real_disk_size = ?env.real_disk_size(),
        "Opened db env"
    );

    Ok(env)
}

/// Create a new env for tests with 1 max named db and a
/// path to a temporary directory.
#[cfg(test)]
pub fn new_test_env() -> (heed::Env, tempfile::TempDir) {
    let tempdir = tempfile::tempdir().unwrap();
    let env = unsafe {
        heed::EnvOpenOptions::new()
            .map_size(100 * 1024 * 1024) // 100 MiB
            .max_dbs(1)
            .flag(heed::Flags::MdbWriteMap)
            .flag(heed::Flags::MdbMapAsync)
            .open(&tempdir)
            .unwrap()
    };
    (env, tempdir)
}
