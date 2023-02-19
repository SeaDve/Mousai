use gtk::{glib, prelude::*, subclass::prelude::*};
use once_cell::unsync::OnceCell;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use std::cell::Cell;

use crate::{
    core::DateTime,
    serde::{
        deserialize_once_cell, deserialize_once_cell_gbytes, serialize_once_cell,
        serialize_once_cell_gbytes,
    },
};

#[derive(Debug, Default, Clone, Copy, glib::Enum, Serialize, Deserialize)]
#[enum_type(name = "MsaiAudioRecordingRecognizeState")]
pub enum RecognizeState {
    #[default]
    Idle,
    Recognizing,
    Done,
}

mod imp {
    use super::*;

    #[derive(Debug, Default, glib::Properties, Serialize, Deserialize)]
    #[properties(wrapper_type = super::Recording)]
    #[serde(default)]
    pub struct Recording {
        #[property(get, set, builder(RecognizeState::default()))]
        pub(super) recognize_state: Cell<RecognizeState>,

        #[serde(
            serialize_with = "serialize_once_cell_gbytes",
            deserialize_with = "deserialize_once_cell_gbytes"
        )]
        pub(super) bytes: OnceCell<glib::Bytes>,
        #[serde(
            serialize_with = "serialize_once_cell",
            deserialize_with = "deserialize_once_cell"
        )]
        pub(super) recorded_time: OnceCell<DateTime>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Recording {
        const NAME: &'static str = "MsaiRecording";
        type Type = super::Recording;
    }

    impl ObjectImpl for Recording {
        crate::derived_properties!();
    }
}

glib::wrapper! {
     pub struct Recording(ObjectSubclass<imp::Recording>);
}

impl Recording {
    pub fn new(bytes: glib::Bytes, recorded_time: DateTime) -> Self {
        let this: Self = glib::Object::new();
        this.imp().bytes.set(bytes).unwrap();
        this.imp().recorded_time.set(recorded_time).unwrap();
        this
    }

    pub fn bytes(&self) -> &glib::Bytes {
        self.imp().bytes.get().unwrap()
    }

    pub fn recorded_time(&self) -> &DateTime {
        self.imp().recorded_time.get().unwrap()
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

        let this: Self = glib::Object::new();
        let imp = this.imp();

        imp.recognize_state
            .set(deserialized_imp.recognize_state.into_inner());

        if let Some(bytes) = deserialized_imp.bytes.into_inner() {
            imp.bytes.set(bytes).unwrap();
        }

        if let Some(recorded_time) = deserialized_imp.recorded_time.into_inner() {
            imp.recorded_time.set(recorded_time).unwrap();
        }

        Ok(this)
    }
}
