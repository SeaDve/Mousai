use chrono::Local;
use gtk::glib;
use serde::{Deserialize, Serialize};

/// A boxed [`DateTime<Local>`](chrono::DateTime<Local>)
#[derive(
    Debug, Clone, Copy, glib::Boxed, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord,
)]
#[boxed_type(name = "NwtyDateTime")]
#[serde(transparent)]
pub struct DateTime(chrono::DateTime<Local>);

impl Default for DateTime {
    fn default() -> Self {
        Self::now()
    }
}

impl DateTime {
    pub fn now() -> Self {
        Self(Local::now())
    }
}
