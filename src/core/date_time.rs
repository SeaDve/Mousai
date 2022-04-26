use gtk::glib;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// A local [`glib::DateTime`] that implements serde traits
#[derive(Debug, Clone, glib::Boxed, PartialEq, Eq, PartialOrd, Ord)]
#[boxed_type(name = "MsaiDateTime")]
pub struct DateTime(glib::DateTime);

impl Default for DateTime {
    fn default() -> Self {
        Self::now()
    }
}

impl DateTime {
    pub fn now() -> Self {
        Self(glib::DateTime::now_local().expect("You are somehow on year 9999"))
    }

    pub fn fuzzy_display(&self) -> String {
        let now = Self::now();

        if self.0.ymd() == now.0.ymd() {
            self.0.format("today at %R") // today at 08:10
        } else if now.0.difference(&self.0).as_hours() <= 30 {
            self.0.format("yesterday at %R") // yesterday at 08:10
        } else {
            self.0.format("%F") // 2001-07-08
        }
        .expect("DateTime formatting error")
        .to_string()
    }

    pub fn format(&self, format: &str) -> Result<glib::GString, glib::BoolError> {
        self.0.format(format)
    }
}

impl Serialize for DateTime {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0
            .format_iso8601()
            .map_err(|_| serde::ser::Error::custom("Failed to format date to iso8601"))?
            .serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for DateTime {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Ok(Self(
            glib::DateTime::from_iso8601(
                &String::deserialize(deserializer)?,
                Some(&glib::TimeZone::local()),
            )
            .map_err(|_| serde::de::Error::custom("Failed to parse date from iso8601"))?,
        ))
    }
}
