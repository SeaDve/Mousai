use ::once_cell::unsync::OnceCell;
use gtk::glib;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

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
        Ok(OnceCell::with_value(T::deserialize(deserializer)?))
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
        Ok(OnceCell::with_value(glib::Bytes::from_owned(
            Vec::<u8>::deserialize(deserializer)?,
        )))
    }
}
