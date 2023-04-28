use gtk::glib;
use heed::types::Str;
use serde::{Deserialize, Serialize};

use std::borrow::Cow;

/// A unique id made up of a namespace and a unique string.
#[derive(Debug, Clone, Hash, PartialEq, Eq, glib::ValueDelegate, Deserialize, Serialize)]
#[serde(transparent)]
pub struct Uid(Box<str>);

impl Uid {
    /// Create an id from the given `unique_str` with the given `namespace`.
    ///
    /// Note: Caller must ensure that `unique_str` is unique for the given `namespace`.
    pub fn from(namespace: &str, unique_str: &str) -> Self {
        Self(format!("{}-{}", namespace, unique_str).into())
    }

    /// Generate a new id with `unique_str` made up of real time and a random u32
    /// both encoded in hex.
    pub fn generate(namespace: &str) -> Self {
        let unique_str = format!("{:x}-{:x}", glib::real_time(), glib::random_int());
        Self::from(namespace, &unique_str)
    }

    /// Create an id from the given `unique_str` with a namespace of "Test".
    #[cfg(test)]
    pub fn for_test(unique_str: &str) -> Self {
        Self::from("Test", unique_str)
    }
}

pub struct UidCodec;

impl heed::BytesEncode<'_> for UidCodec {
    type EItem = Uid;

    fn bytes_encode(item: &Self::EItem) -> Result<Cow<'_, [u8]>, heed::BoxedError> {
        Str::bytes_encode(&item.0)
    }
}

impl<'a> heed::BytesDecode<'a> for UidCodec {
    type DItem = Uid;

    fn bytes_decode(bytes: &'a [u8]) -> Result<Self::DItem, heed::BoxedError> {
        Str::bytes_decode(bytes).map(|s| Uid(s.into()))
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn unique_generated() {
        for i in 0..1_000_000 {
            assert_ne!(
                Uid::generate("Test"),
                Uid::generate("Test"),
                "ids are equal after {} iterations",
                i
            );
        }
    }

    #[test]
    fn equality() {
        assert_eq!(Uid::for_test("A"), Uid::for_test("A"));
        assert_eq!(Uid::for_test("B"), Uid::for_test("B"));

        assert_ne!(Uid::for_test("A"), Uid::for_test("B"));
        assert_ne!(Uid::for_test("A"), Uid::for_test("B"));
    }

    #[test]
    fn serde_bincode() {
        let val = Uid::from("Namespace", "some unique str");
        let bytes = bincode::serialize(&val).unwrap();
        let de_val = bincode::deserialize(&bytes).unwrap();
        assert_eq!(val, de_val);

        let val = Uid::generate("Test");
        let bytes = bincode::serialize(&val).unwrap();
        let de_val = bincode::deserialize(&bytes).unwrap();
        assert_eq!(val, de_val);

        let val = Uid::for_test("b");
        let bytes = bincode::serialize(&val).unwrap();
        let de_val = bincode::deserialize(&bytes).unwrap();
        assert_eq!(val, de_val);
    }

    #[test]
    fn serialize() {
        assert_eq!(
            serde_json::to_string(&Uid::for_test("A")).unwrap().as_str(),
            "\"Test-A\"",
        );
        assert_eq!(
            serde_json::to_string(&Uid::from("Namespace", "BB8"))
                .unwrap()
                .as_str(),
            "\"Namespace-BB8\""
        );
    }

    #[test]
    fn deserialize() {
        assert_eq!(
            Uid::for_test("A"),
            serde_json::from_str("\"Test-A\"").unwrap()
        );
        assert_eq!(
            Uid::from("Namespace", "BB8"),
            serde_json::from_str("\"Namespace-BB8\"").unwrap()
        );
    }

    #[test]
    fn hash_map() {
        let mut hash_map = HashMap::new();

        let id_0 = Uid::for_test("Id0");
        hash_map.insert(&id_0, 0);

        let id_1 = Uid::for_test("Id1");
        hash_map.insert(&id_1, 1);

        let id_2 = Uid::for_test("Id2");
        hash_map.insert(&id_2, 2);

        assert_eq!(hash_map.get(&id_0), Some(&0));
        assert_eq!(hash_map.get(&id_1), Some(&1));
        assert_eq!(hash_map.get(&Uid::for_test("Id2")), Some(&2));
    }
}
