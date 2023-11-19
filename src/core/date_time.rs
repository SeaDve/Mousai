use anyhow::{Context, Result};
use gettextrs::gettext;
use gtk::glib;
use serde::{de, Deserialize, Deserializer, Serialize, Serializer};

/// A local [`glib::DateTime`] that implements [`Serialize`] and [`Deserialize`]
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, glib::ValueDelegate)]
#[value_delegate(nullable)]
pub struct DateTime(glib::DateTime);

impl DateTime {
    pub fn now_local() -> Self {
        Self(glib::DateTime::now_local().unwrap())
    }

    pub fn from_iso8601(string: &str) -> Result<Self> {
        glib::DateTime::from_iso8601(string, Some(&glib::TimeZone::local()))
            .map(Self)
            .with_context(|| format!("Invalid iso8601 datetime `{}`", string))
    }

    pub fn fuzzy_display(&self) -> glib::GString {
        let now = Self::now_local();

        if self.0.ymd() == now.0.ymd() {
            self.0.format(&gettext("today at %R"))
        } else if now.0.difference(&self.0).as_hours() <= 30 {
            self.0.format(&gettext("yesterday at %R"))
        } else {
            self.0.format("%F") // ISO 8601 (e.g., `2001-07-08`)
        }
        .expect("format must be correct")
    }

    pub fn format_iso8601(&self) -> glib::GString {
        self.0.format_iso8601().unwrap()
    }

    pub fn format(&self, format: &str) -> Result<glib::GString> {
        self.0
            .format(format)
            .with_context(|| format!("Failed to format datetime to `{}`", format))
    }
}

impl Serialize for DateTime {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.format_iso8601().serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for DateTime {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let string = <&str>::deserialize(deserializer)?;
        DateTime::from_iso8601(string).map_err(de::Error::custom)
    }
}

impl From<glib::DateTime> for DateTime {
    fn from(dt: glib::DateTime) -> Self {
        Self(dt)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serde_bincode() {
        let val = DateTime::from_iso8601("2022-07-28T08:23:28.623259+08").unwrap();
        let bytes = bincode::serialize(&val).unwrap();
        let de_val = bincode::deserialize(&bytes).unwrap();
        assert_eq!(val, de_val);

        let val = DateTime::now_local();
        let bytes = bincode::serialize(&val).unwrap();
        let de_val = bincode::deserialize(&bytes).unwrap();
        assert_eq!(val, de_val);
    }

    #[test]
    fn serialize() {
        let dt = DateTime::from_iso8601("2022-07-28T08:23:28.623259+08").unwrap();
        assert_eq!(
            serde_json::to_string(&dt).unwrap(),
            "\"2022-07-28T08:23:28.623259+08\"",
        );

        assert_eq!(dt.format_iso8601(), "2022-07-28T08:23:28.623259+08");
    }

    #[test]
    fn deserialize() {
        assert_eq!(
            DateTime::from_iso8601("2022-07-28T08:23:28.623259+08").unwrap(),
            serde_json::from_str("\"2022-07-28T08:23:28.623259+08\"").unwrap()
        );

        assert!(DateTime::from_iso8601("2022").is_err());
        assert!(serde_json::from_str::<DateTime>("\"2022\"").is_err());
    }
}
