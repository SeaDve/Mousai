use gtk::glib;
use serde::{Deserialize, Serialize};

use crate::utils;

#[derive(Debug, Clone, Hash, PartialEq, Eq, glib::Boxed, Deserialize, Serialize)]
#[boxed_type(name = "MsaiSongId")] // TODO drop Boxed derive and replace with ValueDelegate
#[serde(transparent)]
pub struct SongId(Box<str>);

impl SongId {
    /// Note: `unique_str` must be unique to each song.
    pub fn from(namespace: &str, unique_str: &str) -> Self {
        Self(format!("{}-{}", namespace, unique_str).into())
    }

    /// Create a new id from a generated unique string and a namespace of "Mousai".
    ///
    /// Note: This should only be used when an id cannot be properly retrieved.
    pub fn generate_unique() -> Self {
        tracing::warn!("Generating a unique id");

        Self::from("Mousai", &utils::generate_unique_id())
    }

    /// Create a new song id with a namespace of "Test".
    #[cfg(test)]
    pub fn for_test(unique_str: &str) -> Self {
        Self::from("Test", unique_str)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn unique_generated() {
        for i in 0..1000 {
            assert_ne!(
                SongId::generate_unique(),
                SongId::generate_unique(),
                "ids are equal after {} iterations",
                i
            );
        }
    }

    #[test]
    fn equality() {
        assert_eq!(SongId::for_test("A"), SongId::for_test("A"));
        assert_eq!(SongId::for_test("B"), SongId::for_test("B"));

        assert_ne!(SongId::for_test("A"), SongId::for_test("B"));
        assert_ne!(SongId::for_test("A"), SongId::for_test("B"));
    }

    #[test]
    fn serialize() {
        assert_eq!(
            serde_json::to_string(&SongId::for_test("A"))
                .unwrap()
                .as_str(),
            "\"Test-A\"",
        );
        assert_eq!(
            serde_json::to_string(&SongId::from("Namespace", "BB8"))
                .unwrap()
                .as_str(),
            "\"Namespace-BB8\""
        );
    }

    #[test]
    fn deserialize() {
        assert_eq!(
            SongId::for_test("A"),
            serde_json::from_str("\"Test-A\"").unwrap()
        );
        assert_eq!(
            SongId::from("Namespace", "BB8"),
            serde_json::from_str("\"Namespace-BB8\"").unwrap()
        );
    }

    #[test]
    fn hash_map() {
        let mut hash_map = HashMap::new();

        let id_0 = SongId::for_test("Id0");
        hash_map.insert(&id_0, 0);

        let id_1 = SongId::for_test("Id1");
        hash_map.insert(&id_1, 1);

        let id_2 = SongId::for_test("Id2");
        hash_map.insert(&id_2, 2);

        assert_eq!(hash_map.get(&id_0), Some(&0));
        assert_eq!(hash_map.get(&id_1), Some(&1));
        assert_eq!(hash_map.get(&SongId::for_test("Id2")), Some(&2));
    }
}
