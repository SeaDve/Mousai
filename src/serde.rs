use once_cell::unsync::OnceCell;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

pub fn serialize_once_cell<S>(
    cell: &OnceCell<impl Serialize>,
    serializer: S,
) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    cell.get().serialize(serializer)
}

pub fn deserialize_once_cell<'de, D, T>(deserializer: D) -> Result<OnceCell<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    Ok(OnceCell::with_value(T::deserialize(deserializer)?))
}

pub fn serialize_once_cell_gbytes<S>(
    cell: &OnceCell<glib::Bytes>,
    serializer: S,
) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    cell.get().map(|b| b.as_ref()).serialize(serializer)
}

pub fn deserialize_once_cell_gbytes<'de, D>(
    deserializer: D,
) -> Result<OnceCell<glib::Bytes>, D::Error>
where
    D: Deserializer<'de>,
{
    Ok(OnceCell::with_value(glib::Bytes::from_owned(
        Vec::<u8>::deserialize(deserializer)?,
    )))
}
