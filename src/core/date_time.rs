use chrono::Local;
use gtk::glib;
use serde::{Deserialize, Serialize};

/// A boxed [`DateTime<Local>`](chrono::DateTime<Local>)
#[derive(
    Debug, Clone, Copy, glib::Boxed, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord,
)]
#[boxed_type(name = "MsaiDateTime")]
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

    pub fn fuzzy_display(&self) -> String {
        let now = Local::now();

        if self.0.date() == now.date() {
            self.0.format("today at %R") // today at 08:10
        } else if now.signed_duration_since(self.0).num_hours() <= 30 {
            self.0.format("yesterday at %R") // yesterday at 08:10
        } else {
            self.0.format("%F") // 2001-07-08
        }
        .to_string()
    }
}
