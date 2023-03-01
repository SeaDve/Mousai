use gtk::glib;
use serde::{Deserialize, Serialize};

use crate::utils;

#[derive(Debug, Clone, Hash, PartialEq, Eq, glib::Boxed, Deserialize, Serialize)]
#[boxed_type(name = "MsaiSongId")] // TODO drop Boxed derive and replace with ValueDelegate
#[serde(transparent)]
pub struct SongId(Box<str>);

impl SongId {
    /// Note: `unique_str` must be unique to each song.
    pub fn new(namespace: &str, unique_str: &str) -> Self {
        Self(format!("{}-{}", namespace, unique_str).into())
    }

    /// Create a new song id with a namespace of "Test".
    #[cfg(test)]
    pub fn new_for_test(unique_str: &str) -> Self {
        Self::new("Test", unique_str)
    }
}

impl Default for SongId {
    /// Generate a new song id with "Mousai" as the namespace, plus a random "unique" str
    ///
    /// Note: This should only be used when an id cannot be properly retrieved.
    fn default() -> Self {
        tracing::warn!("Using default song id");

        Self::new("Mousai", &utils::generate_unique_id())
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn unique_default() {
        for i in 0..1000 {
            assert_ne!(
                SongId::default(),
                SongId::default(),
                "defaults are equal after {} iterations",
                i
            );
        }
    }

    #[test]
    fn equality() {
        assert_eq!(SongId::new_for_test("A"), SongId::new_for_test("A"));
        assert_eq!(SongId::new_for_test("B"), SongId::new_for_test("B"));

        assert_ne!(SongId::new_for_test("A"), SongId::new_for_test("B"));
        assert_ne!(SongId::new_for_test("A"), SongId::new_for_test("B"));
    }

    #[test]
    fn serialize() {
        assert_eq!(
            serde_json::to_string(&SongId::new_for_test("A"))
                .unwrap()
                .as_str(),
            "\"Test-A\"",
        );
        assert_eq!(
            serde_json::to_string(&SongId::new("Namespace", "BB8"))
                .unwrap()
                .as_str(),
            "\"Namespace-BB8\""
        );
    }

    #[test]
    fn deserialize() {
        assert_eq!(
            SongId::new_for_test("A"),
            serde_json::from_str("\"Test-A\"").unwrap()
        );
        assert_eq!(
            SongId::new("Namespace", "BB8"),
            serde_json::from_str("\"Namespace-BB8\"").unwrap()
        );
    }

    #[test]
    fn hash_map() {
        let mut hash_map = HashMap::new();

        let id_0 = SongId::new_for_test("Id0");
        hash_map.insert(&id_0, 0);

        let id_1 = SongId::new_for_test("Id1");
        hash_map.insert(&id_1, 1);

        let id_2 = SongId::new_for_test("Id2");
        hash_map.insert(&id_2, 2);

        assert_eq!(hash_map.get(&id_0), Some(&0));
        assert_eq!(hash_map.get(&id_1), Some(&1));
        assert_eq!(hash_map.get(&SongId::new_for_test("Id2")), Some(&2));
    }
}
