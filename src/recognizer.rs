use gst_pbutils::prelude::*;
use gtk::{glib, subclass::prelude::*};

use std::{cell::Cell, path::PathBuf};

use crate::{
    core::{AudD, AudioRecorder},
    model::Song,
};

mod imp {
    use super::*;
    use once_cell::sync::Lazy;

    #[derive(Debug, Default)]
    pub struct Recognizer {
        pub is_listening: Cell<bool>,

        pub audio_recorder: AudioRecorder,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Recognizer {
        const NAME: &'static str = "MsaiRecognizer";
        type Type = super::Recognizer;
        type ParentType = glib::Object;
    }

    impl ObjectImpl for Recognizer {
        fn properties() -> &'static [glib::ParamSpec] {
            static PROPERTIES: Lazy<Vec<glib::ParamSpec>> = Lazy::new(|| {
                vec![glib::ParamSpec::new_boolean(
                    "is-listening",
                    "Is Listening",
                    "Whether Self is in listening state",
                    false,
                    glib::ParamFlags::READWRITE | glib::ParamFlags::EXPLICIT_NOTIFY,
                )]
            });
            PROPERTIES.as_ref()
        }

        fn set_property(
            &self,
            obj: &Self::Type,
            _id: usize,
            value: &glib::Value,
            pspec: &glib::ParamSpec,
        ) {
            match pspec.name() {
                "is-listening" => {
                    let is_listening = value.get().unwrap();
                    obj.set_is_listening(is_listening);
                }
                _ => unimplemented!(),
            }
        }

        fn property(&self, obj: &Self::Type, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
            match pspec.name() {
                "is-listening" => obj.is_listening().to_value(),
                _ => unimplemented!(),
            }
        }
    }
}

glib::wrapper! {
    pub struct Recognizer(ObjectSubclass<imp::Recognizer>);
}

impl Recognizer {
    pub fn new() -> Self {
        glib::Object::new::<Self>(&[]).expect("Failed to create Recognizer.")
    }

    pub fn set_is_listening(&self, is_listening: bool) {
        let imp = imp::Recognizer::from_instance(self);
        imp.is_listening.set(is_listening);
        self.notify("is-listening");
    }

    pub fn is_listening(&self) -> bool {
        let imp = imp::Recognizer::from_instance(self);
        imp.is_listening.get()
    }

    pub fn start(&self) -> anyhow::Result<()> {
        let imp = imp::Recognizer::from_instance(self);

        imp.audio_recorder.start(Self::tmp_path())?;
        self.set_is_listening(true);

        Ok(())
    }

    pub async fn stop(&self) -> anyhow::Result<Song> {
        let imp = imp::Recognizer::from_instance(self);

        let recording = imp.audio_recorder.stop().await?;
        self.set_is_listening(false);

        let response = AudD::new(None).recognize(recording.path()).await?;
        Ok(Song::new(
            &response.result.title,
            &response.result.artist,
            &response.result.info_link,
        ))
    }

    fn tmp_path() -> PathBuf {
        let mut tmp_path = glib::tmp_dir();
        tmp_path.push("tmp_recording.ogg");
        tmp_path
    }
}

impl Default for Recognizer {
    fn default() -> Self {
        Self::new()
    }
}
