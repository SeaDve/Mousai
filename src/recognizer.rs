use gst_pbutils::prelude::*;
use gtk::{
    glib::{self, clone},
    prelude::*,
    subclass::prelude::*,
};

use std::{
    cell::{Cell, RefCell},
    path::PathBuf,
};

use crate::{
    core::AudioRecorder,
    model::Song,
    provider::{AudD, Provider},
    Application,
};

#[derive(Debug, Clone, Copy, glib::Enum, PartialEq)]
#[enum_type(name = "MsaiRecognizerState")]
pub enum RecognizerState {
    Null,
    Listening,
    Recognizing,
}

impl Default for RecognizerState {
    fn default() -> Self {
        Self::Null
    }
}

mod imp {
    use super::*;
    use glib::subclass::Signal;
    use once_cell::sync::Lazy;

    pub struct Recognizer {
        pub state: Cell<RecognizerState>,

        pub source_id: RefCell<Option<glib::SourceId>>,
        pub provider: RefCell<Box<dyn Provider>>,
        pub audio_recorder: AudioRecorder,
    }

    impl Default for Recognizer {
        fn default() -> Self {
            Self {
                state: Cell::default(),
                source_id: RefCell::default(),
                provider: RefCell::new(Box::new(AudD::default())),
                audio_recorder: AudioRecorder::default(),
            }
        }
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Recognizer {
        const NAME: &'static str = "MsaiRecognizer";
        type Type = super::Recognizer;
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
                vec![glib::ParamSpecEnum::new(
                    "state",
                    "State",
                    "Current state of Self",
                    RecognizerState::static_type(),
                    RecognizerState::default() as i32,
                    glib::ParamFlags::READABLE,
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
                "state" => {
                    let state = value.get().unwrap();
                    obj.set_state(state);
                }
                _ => unimplemented!(),
            }
        }

        fn property(&self, obj: &Self::Type, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
            match pspec.name() {
                "state" => obj.state().to_value(),
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
    }

    pub fn connect_state_notify<F>(&self, f: F) -> glib::SignalHandlerId
    where
        F: Fn(&Self) + 'static,
    {
        self.connect_notify_local(Some("state"), move |obj, _| f(obj))
    }

    pub fn set_provider(&self, provider: impl Provider + 'static) {
        self.imp().provider.replace(Box::new(provider));
    }

    pub fn state(&self) -> RecognizerState {
        self.imp().state.get()
    }

    pub fn listen(&self) -> anyhow::Result<()> {
        let imp = self.imp();

        let tmp_path = Self::tmp_path();

        log::info!("Saving temporary file at `{}`", tmp_path.display());

        match default_device_name() {
            Ok(ref device_name) => {
                log::info!("Audio recorder setup with device name `{}`", device_name);
                imp.audio_recorder.set_device_name(Some(device_name));
            }
            Err(err) => {
                log::warn!("Failed to get default source name: {:?}", err);
                imp.audio_recorder.set_device_name(None);
            }
        }

        imp.audio_recorder.start(&tmp_path)?;
        self.set_state(RecognizerState::Listening);

        imp.source_id.replace(Some(glib::timeout_add_local_once(
            imp.provider.borrow().listen_duration(),
            clone!(@weak self as obj => move || {
                obj.emit_by_name::<()>("listen-done", &[]);
            }),
        )));

        Ok(())
    }

    #[allow(clippy::await_holding_refcell_ref)]
    pub async fn listen_finish(&self) -> anyhow::Result<Song> {
        let imp = self.imp();

        let recording = imp.audio_recorder.stop().await.map_err(|err| {
            self.set_state(RecognizerState::Null);
            err
        })?;

        let provider = imp.provider.borrow();

        log::debug!("provider: {:?}", provider);

        self.set_state(RecognizerState::Recognizing);

        let song = provider.recognize(&recording).await.map_err(|err| {
            self.set_state(RecognizerState::Null);
            err
        })?;

        self.set_state(RecognizerState::Null);

        Ok(song)
    }

    pub async fn cancel(&self) {
        let imp = self.imp();

        self.set_state(RecognizerState::Null);

        if let Some(source_id) = imp.source_id.take() {
            source_id.remove();
        }

        imp.audio_recorder.cancel().await;
    }

    pub fn audio_recorder(&self) -> &AudioRecorder {
        &self.imp().audio_recorder
    }

    fn set_state(&self, state: RecognizerState) {
        if state == self.state() {
            return;
        }

        self.imp().state.set(state);
        self.notify("state");
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

fn default_device_name() -> anyhow::Result<String> {
    let settings = Application::default().settings();
    let server_info = pulsectl::controllers::SourceController::create()?.get_server_info()?;

    let device_name = match settings.string("preferred-audio-source").as_str() {
        "microphone" => server_info.default_source_name,
        "desktop-audio" => server_info
            .default_sink_name
            .map(|sink_name| format!("{sink_name}.monitor")),
        unknown_device_name => {
            log::warn!(
                "Unknown device name `{unknown_device_name}`. Used default_source_name instead."
            );
            server_info.default_source_name
        }
    };

    device_name.ok_or_else(|| anyhow::anyhow!("Default audio source name not found"))
}
