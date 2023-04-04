use gtk::glib;
use rusqlite::{
    types::{FromSql, FromSqlResult, ToSqlOutput, ValueRef},
    ToSql,
};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use std::ops::Deref;

#[derive(Debug, Clone, PartialEq, Eq, glib::ValueDelegate)]
#[value_delegate(nullable)]
pub struct Bytes(glib::Bytes);

impl Deref for Bytes {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<&'static [u8]> for Bytes {
    fn from(value: &'static [u8]) -> Self {
        Self(glib::Bytes::from_static(value))
    }
}

impl From<glib::Bytes> for Bytes {
    fn from(value: glib::Bytes) -> Self {
        Self(value)
    }
}

impl FromSql for Bytes {
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        let bytes = value.as_blob()?;
        Ok(Self(glib::Bytes::from(bytes)))
    }
}

impl ToSql for Bytes {
    fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
        Ok(ToSqlOutput::from(self.0.as_ref()))
    }
}

impl Serialize for Bytes {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Bytes {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let bytes = Vec::<u8>::deserialize(deserializer)?;
        Ok(Self(glib::Bytes::from_owned(bytes)))
    }
}
