use gtk::glib;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use std::cell::OnceCell;

pub mod once_cell {
    use super::*;

    pub fn serialize<S>(cell: &OnceCell<impl Serialize>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        cell.get().serialize(serializer)
    }

    pub fn deserialize<'de, D, T>(deserializer: D) -> Result<OnceCell<T>, D::Error>
    where
        D: Deserializer<'de>,
        T: Deserialize<'de>,
    {
        let val = Option::<T>::deserialize(deserializer)?;
        Ok(val.map_or_else(OnceCell::new, OnceCell::from))
    }
}

pub mod once_cell_gbytes {
    use super::*;

    pub fn serialize<S>(cell: &OnceCell<glib::Bytes>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        cell.get().map(|bytes| bytes.as_ref()).serialize(serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<OnceCell<glib::Bytes>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let val = Option::<Vec<u8>>::deserialize(deserializer)?;
        Ok(val.map_or_else(OnceCell::new, |val| {
            OnceCell::from(glib::Bytes::from_owned(val))
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
    struct Test {
        #[serde(with = "once_cell")]
        once_cell: OnceCell<i32>,
        #[serde(with = "once_cell_gbytes")]
        once_cell_gbytes: OnceCell<glib::Bytes>,
    }

    #[test]
    fn serde_bincode() {
        let val = Test {
            once_cell: OnceCell::new(),
            once_cell_gbytes: OnceCell::new(),
        };
        let bytes = bincode::serialize(&val).unwrap();
        let de_val = bincode::deserialize::<Test>(&bytes).unwrap();
        assert_eq!(val, de_val);

        let val = Test {
            once_cell: OnceCell::from(100),
            once_cell_gbytes: OnceCell::from(glib::Bytes::from_owned(vec![0])),
        };
        let bytes = bincode::serialize(&val).unwrap();
        let de_val = bincode::deserialize::<Test>(&bytes).unwrap();
        assert_eq!(val, de_val);
    }
}
