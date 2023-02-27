use gtk::glib;
use once_cell::sync::Lazy;

use crate::core::Database;

pub fn connection() -> Database {
    static DATABASE: Lazy<Database> = Lazy::new(|| {
        let path = glib::home_dir().join("mousai.db");
        Database::open(path).unwrap()
    });

    DATABASE.clone()
}
