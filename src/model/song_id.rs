use gtk::glib;
use serde::{Deserialize, Serialize};

use std::fmt;

#[derive(Debug, Clone, Hash, PartialEq, Eq, glib::Boxed, Deserialize, Serialize)]
#[boxed_type(name = "MsaiSongId")]
#[serde(transparent)]
pub struct SongId(Box<str>);

impl fmt::Display for SongId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.0, f)
    }
}

impl Default for SongId {
    /// This is only used when a song is constructed and will serve as a
    /// failsafe when a proper ID was not set in the constructor of a `Song`.
    /// Thus, it must still be unique.
    fn default() -> Self {
        Self::from(format!("Default-{}-Default", glib::real_time()))
    }
}

impl SongId {
    /// This must be unique to every song.
    ///
    /// Note: Unique str must not start and end with `Default`.
    pub fn from(unique_str: impl Into<Box<str>>) -> Self {
        Self(unique_str.into())
    }

    pub fn is_default(&self) -> bool {
        self.0.starts_with("Default") && self.0.ends_with("Default")
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn default() {
        assert!(SongId::default().is_default());
        assert!(!SongId::from("A").is_default());
    }

    #[test]
    fn unique_default() {
        assert_ne!(SongId::default(), SongId::default());
        assert_ne!(SongId::default(), SongId::default());
    }

    #[test]
    fn equality() {
        assert_eq!(SongId::from("A"), SongId::from("A"));
        assert_eq!(SongId::from("B"), SongId::from("B"));

        assert_ne!(SongId::from("A"), SongId::from("B"));
        assert_ne!(SongId::from("A"), SongId::from("B"));
    }

    #[test]
    fn serialize() {
        assert_eq!(
            serde_json::to_string(&SongId::from("A")).unwrap().as_str(),
            "\"A\"",
        );

        assert_eq!(
            serde_json::to_string(&SongId::from("BB8"))
                .unwrap()
                .as_str(),
            "\"BB8\""
        );
    }

    #[test]
    fn deserialize() {
        assert_eq!(SongId::from("A"), serde_json::from_str("\"A\"").unwrap());
        assert_eq!(
            SongId::from("BB8"),
            serde_json::from_str("\"BB8\"").unwrap()
        );
    }

    #[test]
    fn hash_map() {
        let mut hash_map = HashMap::new();

        let id_0 = SongId::from("Id0");
        hash_map.insert(&id_0, 0);

        let id_1 = SongId::from("Id1");
        hash_map.insert(&id_1, 1);

        let id_2 = SongId::from("Id2");
        hash_map.insert(&id_2, 2);

        assert_eq!(hash_map.get(&id_0), Some(&0));
        assert_eq!(hash_map.get(&id_1), Some(&1));
        assert_eq!(hash_map.get(&SongId::from("Id2")), Some(&2));
    }
}
