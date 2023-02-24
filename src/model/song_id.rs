use gtk::glib;
use serde::{Deserialize, Serialize};

use std::fmt;

use crate::core::DateTime;

#[derive(Debug, Clone, Hash, PartialEq, Eq, glib::Boxed, Deserialize, Serialize)]
#[boxed_type(name = "MsaiSongId")] // TODO drop Boxed derive and replace with ValueDelegate
#[serde(transparent)]
pub struct SongId(Box<str>);

impl fmt::Display for SongId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.0, f)
    }
}

impl SongId {
    /// This must be unique to every song.
    pub fn new(unique_str: impl Into<Box<str>>) -> Self {
        Self(unique_str.into())
    }
}

impl Default for SongId {
    /// Generate a new song id with time stamp based on the current time.
    ///
    /// Note: This should only be used when an id cannot be properly retrieved.
    fn default() -> Self {
        tracing::warn!("Using default song id");

        Self::new(
            DateTime::now_local()
                .format("MsaiSongId-%Y-%m-%d-%H-%M-%S-%f")
                .unwrap(),
        )
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn unique_default() {
        assert_ne!(SongId::default(), SongId::default());
        assert_ne!(SongId::default(), SongId::default());
        assert_ne!(SongId::default(), SongId::default());
    }

    #[test]
    fn equality() {
        assert_eq!(SongId::new("A"), SongId::new("A"));
        assert_eq!(SongId::new("B"), SongId::new("B"));

        assert_ne!(SongId::new("A"), SongId::new("B"));
        assert_ne!(SongId::new("A"), SongId::new("B"));
    }

    #[test]
    fn serialize() {
        assert_eq!(
            serde_json::to_string(&SongId::new("A")).unwrap().as_str(),
            "\"A\"",
        );

        assert_eq!(
            serde_json::to_string(&SongId::new("BB8")).unwrap().as_str(),
            "\"BB8\""
        );
    }

    #[test]
    fn deserialize() {
        assert_eq!(SongId::new("A"), serde_json::from_str("\"A\"").unwrap());
        assert_eq!(SongId::new("BB8"), serde_json::from_str("\"BB8\"").unwrap());
    }

    #[test]
    fn hash_map() {
        let mut hash_map = HashMap::new();

        let id_0 = SongId::new("Id0");
        hash_map.insert(&id_0, 0);

        let id_1 = SongId::new("Id1");
        hash_map.insert(&id_1, 1);

        let id_2 = SongId::new("Id2");
        hash_map.insert(&id_2, 2);

        assert_eq!(hash_map.get(&id_0), Some(&0));
        assert_eq!(hash_map.get(&id_1), Some(&1));
        assert_eq!(hash_map.get(&SongId::new("Id2")), Some(&2));
    }
}
