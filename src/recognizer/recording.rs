use gtk::{glib, prelude::*, subclass::prelude::*};
use serde::{de, Deserialize, Deserializer, Serialize, Serializer};

use std::cell::{Cell, OnceCell, RefCell};

use super::RecognizeError;
use crate::{date_time::DateTime, serde_helpers, song::Song};

#[derive(Debug, Clone, PartialEq, Eq, glib::Boxed, Serialize, Deserialize)]
#[boxed_type(name = "MsaiBoxedRecognizeResult", nullable)]
pub struct BoxedRecognizeResult(pub Result<Song, RecognizeError>);

mod imp {
    use super::*;

    #[derive(Default, glib::Properties, Serialize, Deserialize)]
    #[properties(wrapper_type = super::Recording)]
    pub struct Recording {
        #[property(get, set, construct_only)]
        #[serde(with = "serde_helpers::once_cell_gbytes")]
        pub(super) bytes: OnceCell<glib::Bytes>,
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

    #[glib::derived_properties]
    impl ObjectImpl for Recording {}

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
    pub fn new(bytes: &glib::Bytes, recorded_time: &DateTime) -> Self {
        glib::Object::builder()
            .property("bytes", bytes)
            .property("recorded-time", recorded_time)
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
        Ok(glib::Object::builder()
            .property(
                "bytes",
                deserialized_imp
                    .bytes
                    .into_inner()
                    .ok_or_else(|| de::Error::missing_field("bytes"))?,
            )
            .property(
                "recorded-time",
                deserialized_imp
                    .recorded_time
                    .into_inner()
                    .ok_or_else(|| de::Error::missing_field("recorded_time"))?,
            )
            .property(
                "recognize-result",
                deserialized_imp.recognize_result.into_inner(),
            )
            .build())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::recognizer::RecognizeErrorKind;

    fn assert_recording_eq(v1: &Recording, v2: &Recording) {
        assert_eq!(v1.bytes(), v2.bytes());
        assert_eq!(v1.recorded_time(), v2.recorded_time());
        assert_eq!(v1.recognize_result(), v2.recognize_result());
    }

    #[test]
    fn serde_bincode() {
        let val = Recording::new(&glib::Bytes::from_static(b""), &DateTime::now_utc());
        let bytes = bincode::serialize(&val).unwrap();
        let de_val = bincode::deserialize::<Recording>(&bytes).unwrap();
        assert_recording_eq(&val, &de_val);
        assert_eq!(val.recognize_retries(), de_val.recognize_retries());

        let val = Recording::new(&glib::Bytes::from_static(b"a"), &DateTime::now_utc());
        let bytes = bincode::serialize(&val).unwrap();
        let de_val = bincode::deserialize::<Recording>(&bytes).unwrap();
        assert_recording_eq(&val, &de_val);
        assert_eq!(val.recognize_retries(), de_val.recognize_retries());

        let val = Recording::new(&glib::Bytes::from_owned(vec![]), &DateTime::now_utc());
        let bytes = bincode::serialize(&val).unwrap();
        let de_val = bincode::deserialize::<Recording>(&bytes).unwrap();
        assert_recording_eq(&val, &de_val);
        assert_eq!(val.recognize_retries(), de_val.recognize_retries());

        let val = Recording::new(&glib::Bytes::from_owned(vec![1]), &DateTime::now_utc());
        let bytes = bincode::serialize(&val).unwrap();
        let de_val = bincode::deserialize::<Recording>(&bytes).unwrap();
        assert_recording_eq(&val, &de_val);
        assert_eq!(val.recognize_retries(), de_val.recognize_retries());

        let val = Recording::new(&glib::Bytes::from_owned(vec![1, 2]), &DateTime::now_utc());
        let bytes = bincode::serialize(&val).unwrap();
        let de_val = bincode::deserialize::<Recording>(&bytes).unwrap();
        assert_recording_eq(&val, &de_val);
        assert_eq!(val.recognize_retries(), de_val.recognize_retries());

        let val = Recording::new(&glib::Bytes::from_owned(vec![1, 2]), &DateTime::now_utc());
        val.set_recognize_result(Some(BoxedRecognizeResult(Err(RecognizeError::new(
            RecognizeErrorKind::Connection,
            "Some message".to_string(),
        )))));
        let bytes = bincode::serialize(&val).unwrap();
        let de_val = bincode::deserialize::<Recording>(&bytes).unwrap();
        assert_recording_eq(&val, &de_val);
        assert_eq!(val.recognize_retries(), de_val.recognize_retries());

        let val = Recording::new(&glib::Bytes::from_owned(vec![1, 2]), &DateTime::now_utc());
        val.increment_recognize_retries();
        let bytes = bincode::serialize(&val).unwrap();
        let de_val = bincode::deserialize::<Recording>(&bytes).unwrap();
        assert_recording_eq(&val, &de_val);
        assert_ne!(val.recognize_retries(), de_val.recognize_retries());
    }
}
