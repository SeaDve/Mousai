use anyhow::{Context, Result};
use gettextrs::gettext;
use gtk::glib;
use serde::{de, Deserialize, Deserializer, Serialize, Serializer};

use std::fmt;

/// A local [`glib::DateTime`] that implements [`Serialize`] and [`Deserialize`]
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

    pub fn parse_from_iso8601(string: &str) -> Result<Self> {
        glib::DateTime::from_iso8601(string, Some(&glib::TimeZone::local()))
            .map(Self)
            .with_context(|| format!("Failed to parse `{}`", string))
    }

    pub fn fuzzy_display(&self) -> glib::GString {
        let now = Self::now();

        if self.0.ymd() == now.0.ymd() {
            // Translators: `%R` will be replaced with 24-hour formatted date time (e.g., `13:21`)
            self.0.format(&gettext("today at %R"))
        } else if now.0.difference(&self.0).as_hours() <= 30 {
            // Translators: `%R` will be replaced with 24-hour formatted date time (e.g., `13:21`)
            self.0.format(&gettext("yesterday at %R"))
        } else {
            self.0.format("%F") // ISO 8601 (e.g., `2001-07-08`)
        }
        .expect("DateTime formatting error")
    }

    pub fn to_iso8601(&self) -> glib::GString {
        self.0
            .format_iso8601()
            .expect("Failed to format date to iso6801")
    }

    pub fn format(&self, format: &str) -> Result<glib::GString> {
        self.0
            .format(format)
            .with_context(|| format!("Failed to format to `{}`", format))
    }
}

impl Serialize for DateTime {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_iso8601())
    }
}

struct DateTimeVisitor;

impl<'de> de::Visitor<'de> for DateTimeVisitor {
    type Value = DateTime;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("an iso8601 formatted date and time string")
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        DateTime::parse_from_iso8601(value)
            .map_err(|_| de::Error::custom("Failed to parse date from iso8601"))
    }
}

impl<'de> Deserialize<'de> for DateTime {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_str(DateTimeVisitor)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serialize() {
        let dt = DateTime::parse_from_iso8601("2022-07-28T08:23:28.623259+08").unwrap();
        assert_eq!(
            serde_json::to_string(&dt).unwrap(),
            "\"2022-07-28T08:23:28.623259+08\"",
        );

        assert_eq!(dt.to_iso8601(), "2022-07-28T08:23:28.623259+08");
    }

    #[test]
    fn deserialize() {
        assert_eq!(
            DateTime::parse_from_iso8601("2022-07-28T08:23:28.623259+08").unwrap(),
            serde_json::from_str("\"2022-07-28T08:23:28.623259+08\"").unwrap()
        );

        assert!(DateTime::parse_from_iso8601("2022").is_err());
        assert!(serde_json::from_str::<DateTime>("\"2022\"").is_err());
    }
}
