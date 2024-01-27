use gtk::glib;
use heed::types::Str;
use serde::{Deserialize, Serialize};

use std::borrow::Cow;

/// A unique id made up of a namespace and a unique string.
#[derive(Debug, Clone, Hash, PartialEq, Eq, glib::ValueDelegate, Deserialize, Serialize)]
#[serde(transparent)]
pub struct Uid(Box<str>);

impl Uid {
    /// Create an id from the given `unique_str`.
    ///
    /// Note: Caller must ensure that `unique_str` is unique.
    pub fn from(unique_str: impl Into<Box<str>>) -> Self {
        Self(unique_str.into())
    }

    /// Create an id from the given `unique_str` prefixed with `prefix` and
    /// joined with a `-`.
    ///
    /// Note: Caller must ensure that `unique_str` is unique in the context of
    /// the given `prefix`.
    pub fn from_prefixed(prefix: &str, unique_str: &str) -> Self {
        Self::from(format!("{}-{}", prefix, unique_str))
    }

    /// Generate a new id with `unique_str` made up of real time and a random u32
    /// both encoded in hex.
    pub fn generate() -> Self {
        Self::from(format!("{:x}-{:x}", glib::real_time(), glib::random_int()))
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
        Str::bytes_decode(bytes).map(Uid::from)
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
                Uid::generate(),
                Uid::generate(),
                "ids are equal after {} iterations",
                i
            );
        }
    }

    #[test]
    fn equality() {
        assert_eq!(Uid::from("A"), Uid::from("A"));
        assert_eq!(Uid::from("B"), Uid::from("B"));

        assert_ne!(Uid::from("A"), Uid::from("B"));
        assert_ne!(Uid::from("A"), Uid::from("B"));
    }

    #[test]
    fn serde_bincode() {
        let val = Uid::from("some unique str");
        let bytes = bincode::serialize(&val).unwrap();
        let de_val = bincode::deserialize(&bytes).unwrap();
        assert_eq!(val, de_val);

        let val = Uid::from_prefixed("a", "b");
        let bytes = bincode::serialize(&val).unwrap();
        let de_val = bincode::deserialize(&bytes).unwrap();
        assert_eq!(val, de_val);

        let val = Uid::generate();
        let bytes = bincode::serialize(&val).unwrap();
        let de_val = bincode::deserialize(&bytes).unwrap();
        assert_eq!(val, de_val);
    }

    #[test]
    fn serialize() {
        assert_eq!(
            serde_json::to_string(&Uid::from("A")).unwrap().as_str(),
            "\"A\"",
        );
        assert_eq!(
            serde_json::to_string(&Uid::from_prefixed("a", "BB8"))
                .unwrap()
                .as_str(),
            "\"a-BB8\""
        );
    }

    #[test]
    fn deserialize() {
        assert_eq!(Uid::from("A"), serde_json::from_str("\"A\"").unwrap());
        assert_eq!(
            Uid::from_prefixed("a", "BB8"),
            serde_json::from_str("\"a-BB8\"").unwrap()
        );
    }

    #[test]
    fn hash_map() {
        let mut hash_map = HashMap::new();

        let id_0 = Uid::from("Id0");
        hash_map.insert(&id_0, 0);

        let id_1 = Uid::from("Id1");
        hash_map.insert(&id_1, 1);

        let id_2 = Uid::from("Id2");
        hash_map.insert(&id_2, 2);

        assert_eq!(hash_map.get(&id_0), Some(&0));
        assert_eq!(hash_map.get(&id_1), Some(&1));
        assert_eq!(hash_map.get(&Uid::from("Id2")), Some(&2));
    }
}
