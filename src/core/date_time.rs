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

        let is_today = now.date() == self.0.date();
        let duration = now.signed_duration_since(self.0);

        let hours_difference = duration.num_hours();
        let week_difference = duration.num_weeks();

        if is_today {
            self.0.format("%I∶%M") // 08:10
        } else if hours_difference <= 30 {
            self.0.format("yesterday")
        } else if week_difference <= 52 {
            self.0.format("%B %d") // September 03
        } else {
            self.0.format("%B %d %Y") // September 03 1920
        }
        .to_string()
    }
}
