use gst_pbutils::prelude::*;
use gtk::{
    glib::{self, clone},
    subclass::prelude::*,
};

use std::{
    cell::{Cell, RefCell},
    path::PathBuf,
    time::Duration,
};

use crate::{
    core::{AudD, AudioRecorder},
    model::Song,
};

const DEFAULT_LISTEN_DURATION: Duration = Duration::from_secs(5);

mod imp {
    use super::*;
    use glib::subclass::Signal;
    use once_cell::sync::Lazy;

    #[derive(Debug, Default)]
    pub struct Recognizer {
        pub is_listening: Cell<bool>,

        pub source_id: RefCell<Option<glib::SourceId>>,
        pub audio_recorder: AudioRecorder,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Recognizer {
        const NAME: &'static str = "MsaiRecognizer";
        type Type = super::Recognizer;
        type ParentType = glib::Object;
    }

    impl ObjectImpl for Recognizer {
        fn signals() -> &'static [Signal] {
            static SIGNALS: Lazy<Vec<Signal>> = Lazy::new(|| {
                vec![Signal::builder("listen-done", &[], <()>::static_type().into()).build()]
            });
            SIGNALS.as_ref()
        }

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

    pub fn connect_listen_done<F>(&self, f: F) -> glib::SignalHandlerId
    where
        F: Fn(&Self) + 'static,
    {
        self.connect_local("listen-done", true, move |values| {
            let obj = values[0].get::<Self>().unwrap();
            f(&obj);
            None
        })
        .unwrap()
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

    pub fn listen(&self) -> anyhow::Result<()> {
        let imp = imp::Recognizer::from_instance(self);

        imp.audio_recorder.start(Self::tmp_path())?;
        self.set_is_listening(true);

        imp.source_id.replace(Some(glib::timeout_add_local_once(
            DEFAULT_LISTEN_DURATION,
            clone!(@weak self as obj => move || {
                obj.emit_by_name("listen-done", &[]).unwrap();
            }),
        )));

        Ok(())
    }

    pub async fn listen_finish(&self) -> anyhow::Result<Song> {
        let imp = imp::Recognizer::from_instance(self);

        let recording = imp.audio_recorder.stop().await?;

        // TODO add a trait that has `recognize` method that returns a Song
        let response = AudD::new(None).recognize(recording.path()).await?;

        self.set_is_listening(false);

        response
            .data
            .map(|data| Song::new(&data.title, &data.artist, &data.info_link))
            .ok_or_else(|| {
                // TODO handle this in AudD recognize method
                anyhow::anyhow!("Cannot recognize song")
            })
    }

    pub async fn cancel(&self) {
        let imp = imp::Recognizer::from_instance(self);

        self.set_is_listening(false);

        if let Some(source_id) = imp.source_id.take() {
            glib::source_remove(source_id);
        }

        imp.audio_recorder.cancel().await;
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
