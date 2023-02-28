use anyhow::Result;
use gtk::glib;

use std::fs;

pub const SONG_LIST_DB_NAME: &str = "song_list";
pub const RECORDINGS_DB_NAME: &str = "saved_recordings";

const N_DB: u32 = 2;

pub fn init_env() -> Result<heed::Env> {
    let path = glib::user_data_dir().join("mousai/db");
    fs::create_dir_all(&path)?;
    let env = heed::EnvOpenOptions::new().max_dbs(N_DB).open(path)?;
    Ok(env)
}
