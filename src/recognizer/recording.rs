use gtk::{glib, prelude::*, subclass::prelude::*};
use once_cell::unsync::OnceCell;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use std::cell::{Cell, RefCell};

use super::RecognizeError;
use crate::{
    core::{Bytes, DateTime},
    model::Song,
    serde_helpers,
};

#[derive(Debug, Clone, PartialEq, Eq, glib::Boxed, Serialize, Deserialize)]
#[boxed_type(name = "MsaiBoxedRecognizeResult", nullable)]
pub struct BoxedRecognizeResult(pub Result<Song, RecognizeError>);

/// Returns a boxed result from the given ok and err values.
/// Returns an `Err` if both are `Some`.
// pub fn create_recognize_result(
//     ok: Option<SongId>,
//     err: Option<RecognizeError>,
// ) -> rusqlite::Result<Option<BoxedRecognizeResult>> {
//     match (ok, err) {
//         (Some(ok), None) => Ok(Some(BoxedRecognizeResult(Ok(ok)))),
//         (None, Some(err)) => Ok(Some(BoxedRecognizeResult(Err(err)))),
//         (None, None) => Ok(None),
//         (Some(_), Some(_)) => Err(rusqlite::Error::FromSqlConversionFailure(
//             usize::MAX,
//             rusqlite::types::Type::Text,
//             "Both ok and err are Some".into(),
//         )),
//     }
// }

mod imp {
    use super::*;

    #[derive(Debug, Default, glib::Properties, Serialize, Deserialize)]
    #[properties(wrapper_type = super::Recording)]
    pub struct Recording {
        #[property(get, set, construct_only)]
        #[serde(with = "serde_helpers::once_cell")]
        pub(super) id: OnceCell<String>,
        #[property(get, set, construct_only)]
        #[serde(with = "serde_helpers::once_cell")]
        pub(super) bytes: OnceCell<Bytes>,
        #[property(get, set, construct_only)]
        #[serde(with = "serde_helpers::once_cell")]
        pub(super) recorded_time: OnceCell<DateTime>,
        #[property(get, set = Self::set_recognize_result, explicit_notify, nullable)]
        pub(super) recognize_result: RefCell<Option<BoxedRecognizeResult>>,

        #[serde(skip)] // So we can retry next session
        pub(super) recognize_retries: Cell<u8>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Recording {
        const NAME: &'static str = "MsaiRecording";
        type Type = super::Recording;
    }

    impl ObjectImpl for Recording {
        crate::derived_properties!();
    }

    impl Recording {
        fn set_recognize_result(&self, result: Option<BoxedRecognizeResult>) {
            if result.as_ref() == self.recognize_result.borrow().as_ref() {
                return;
            }

            let obj = self.obj();
            self.recognize_result.replace(result);
            obj.notify_recognize_result();
        }
    }
}

glib::wrapper! {
     pub struct Recording(ObjectSubclass<imp::Recording>);
}

impl Recording {
    pub fn new(id: &str, bytes: &Bytes, recorded_time: &DateTime) -> Self {
        glib::Object::builder()
            .property("id", id)
            .property("bytes", bytes)
            .property("recorded-time", recorded_time)
            .build()
    }

    pub fn from_raw_parts(
        id: String,
        bytes: Bytes,
        recorded_time: DateTime,
        recognize_result: Option<BoxedRecognizeResult>,
    ) -> Self {
        glib::Object::builder()
            .property("id", id)
            .property("bytes", bytes)
            .property("recorded-time", recorded_time)
            .property("recognize-result", recognize_result)
            .build()
    }

    pub fn recognize_retries(&self) -> u8 {
        self.imp().recognize_retries.get()
    }

    pub fn increment_recognize_retries(&self) {
        let imp = self.imp();
        imp.recognize_retries.set(imp.recognize_retries.get() + 1);
    }
}

impl Serialize for Recording {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.imp().serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Recording {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let deserialized_imp = imp::Recording::deserialize(deserializer)?;
        Ok(Self::from_raw_parts(
            deserialized_imp
                .id
                .into_inner()
                .ok_or_else(|| serde::de::Error::missing_field("id"))?,
            deserialized_imp
                .bytes
                .into_inner()
                .ok_or_else(|| serde::de::Error::missing_field("bytes"))?,
            deserialized_imp
                .recorded_time
                .into_inner()
                .ok_or_else(|| serde::de::Error::missing_field("recorded_time"))?,
            deserialized_imp.recognize_result.into_inner(),
        ))
    }
}

// impl database::Parameterizable for Recording {
//     type Params = (
//         String,
//         Vec<u8>,
//         DateTime,
//         Option<SongId>,
//         Option<RecognizeError>,
//     );

//     fn param_values(&self) -> Self::Params {
//         (
//             self.id(),
//             self.bytes().to_vec(),
//             self.recorded_time(),
//             self.recognize_result().and_then(|r| r.0.ok()),
//             self.recognize_result().and_then(|r| r.0.err()),
//         )
//     }

//     fn param_fields() -> &'static str {
//         "id, bytes, recorded_time, recognize_result_ok, recognize_result_err"
//     }

//     fn param_len() -> usize {
//         5
//     }
// }

#[cfg(test)]
mod tests {
    use super::*;

    use crate::recognizer::RecognizeErrorKind;

    fn assert_recording_eq(v1: &Recording, v2: &Recording) {
        assert_eq!(v1.bytes(), v2.bytes());
        assert_eq!(v1.recorded_time(), v2.recorded_time());
        assert_eq!(v1.recognize_result(), v2.recognize_result());
    }

    // #[test]
    // fn serde_bincode() {
    //     let val = Recording::new(&glib::Bytes::from_static(b""), &DateTime::now_local());
    //     let bytes = bincode::serialize(&val).unwrap();
    //     let de_val = bincode::deserialize::<Recording>(&bytes).unwrap();
    //     assert_recording_eq(&val, &de_val);
    //     assert_eq!(val.recognize_retries(), de_val.recognize_retries());

    //     let val = Recording::new(&glib::Bytes::from_static(b"a"), &DateTime::now_local());
    //     let bytes = bincode::serialize(&val).unwrap();
    //     let de_val = bincode::deserialize::<Recording>(&bytes).unwrap();
    //     assert_recording_eq(&val, &de_val);
    //     assert_eq!(val.recognize_retries(), de_val.recognize_retries());

    //     let val = Recording::new(&glib::Bytes::from_owned(vec![]), &DateTime::now_local());
    //     let bytes = bincode::serialize(&val).unwrap();
    //     let de_val = bincode::deserialize::<Recording>(&bytes).unwrap();
    //     assert_recording_eq(&val, &de_val);
    //     assert_eq!(val.recognize_retries(), de_val.recognize_retries());

    //     let val = Recording::new(&glib::Bytes::from_owned(vec![1]), &DateTime::now_local());
    //     let bytes = bincode::serialize(&val).unwrap();
    //     let de_val = bincode::deserialize::<Recording>(&bytes).unwrap();
    //     assert_recording_eq(&val, &de_val);
    //     assert_eq!(val.recognize_retries(), de_val.recognize_retries());

    //     let val = Recording::new(&glib::Bytes::from_owned(vec![1, 2]), &DateTime::now_local());
    //     let bytes = bincode::serialize(&val).unwrap();
    //     let de_val = bincode::deserialize::<Recording>(&bytes).unwrap();
    //     assert_recording_eq(&val, &de_val);
    //     assert_eq!(val.recognize_retries(), de_val.recognize_retries());

    //     let val = Recording::new(&glib::Bytes::from_owned(vec![1, 2]), &DateTime::now_local());
    //     val.set_recognize_result(Some(BoxedRecognizeResult(Err(RecognizeError::new(
    //         RecognizeErrorKind::Connection,
    //         "Some message".to_string(),
    //     )))));
    //     let bytes = bincode::serialize(&val).unwrap();
    //     let de_val = bincode::deserialize::<Recording>(&bytes).unwrap();
    //     assert_recording_eq(&val, &de_val);
    //     assert_eq!(val.recognize_retries(), de_val.recognize_retries());

    //     let val = Recording::new(&glib::Bytes::from_owned(vec![1, 2]), &DateTime::now_local());
    //     val.increment_recognize_retries();
    //     let bytes = bincode::serialize(&val).unwrap();
    //     let de_val = bincode::deserialize::<Recording>(&bytes).unwrap();
    //     assert_recording_eq(&val, &de_val);
    //     assert_ne!(val.recognize_retries(), de_val.recognize_retries());
    // }
}
