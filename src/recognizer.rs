use gettextrs::gettext;
use gst_pbutils::prelude::*;
use gtk::{
    glib::{self, clone},
    prelude::*,
    subclass::prelude::*,
};
use once_cell::sync::Lazy;

use std::{
    cell::{Cell, RefCell},
    path::PathBuf,
};

use crate::{
    core::{AudioRecorder, Cancellable, Cancelled},
    model::Song,
    provider::PROVIDER_MANAGER,
    spawn, utils, Application,
};

static TMP_RECORDING_PATH: Lazy<PathBuf> = Lazy::new(|| {
    let mut tmp_path = glib::tmp_dir();
    tmp_path.push("tmp_recording.ogg");
    tmp_path
});

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

    #[derive(Debug, Default)]
    pub struct Recognizer {
        pub state: Cell<RecognizerState>,

        pub audio_recorder: AudioRecorder,
        pub cancellable: RefCell<Option<Cancellable>>,
        pub source_id: RefCell<Option<glib::SourceId>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Recognizer {
        const NAME: &'static str = "MsaiRecognizer";
        type Type = super::Recognizer;
    }

    impl ObjectImpl for Recognizer {
        fn signals() -> &'static [Signal] {
            static SIGNALS: Lazy<Vec<Signal>> = Lazy::new(|| {
                vec![Signal::builder(
                    "song-recognized",
                    &[Song::static_type().into()],
                    <()>::static_type().into(),
                )
                .build()]
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

    pub fn connect_song_recognized<F>(&self, f: F) -> glib::SignalHandlerId
    where
        F: Fn(&Self, &Song) + 'static,
    {
        self.connect_local("song-recognized", true, move |values| {
            let obj = values[0].get::<Self>().unwrap();
            let song = values[1].get::<Song>().unwrap();
            f(&obj, &song);
            None
        })
    }

    pub fn connect_state_notify<F>(&self, f: F) -> glib::SignalHandlerId
    where
        F: Fn(&Self) + 'static,
    {
        self.connect_notify_local(Some("state"), move |obj, _| f(obj))
    }

    pub fn state(&self) -> RecognizerState {
        self.imp().state.get()
    }

    pub fn audio_recorder(&self) -> &AudioRecorder {
        &self.imp().audio_recorder
    }

    pub async fn toggle_recognize(&self) -> anyhow::Result<()> {
        let imp = self.imp();

        match self.state() {
            RecognizerState::Listening | RecognizerState::Recognizing => {
                if let Some(cancellable) = imp.cancellable.take() {
                    cancellable.cancel();
                }
            }
            RecognizerState::Null => {
                let cancellable = Cancellable::default();
                imp.cancellable
                    .replace(Some(Cancellable::clone(&cancellable)));

                if let Err(err) = self.recognize(&cancellable).await {
                    if let Some(cancelled) = err.downcast_ref::<Cancelled>() {
                        log::info!("{}", cancelled);
                    } else {
                        return Err(err);
                    }
                }
            }
        }

        Ok(())
    }

    async fn recognize(&self, cancellable: &Cancellable) -> anyhow::Result<()> {
        self.update_audio_recorder_device_name();

        let imp = self.imp();

        log::info!(
            "Saving temporary file at `{}`",
            TMP_RECORDING_PATH.display()
        );

        if let Err(err) = imp.audio_recorder.start(TMP_RECORDING_PATH.as_path()) {
            self.set_state(RecognizerState::Null);
            return Err(err);
        }

        self.set_state(RecognizerState::Listening);
        let provider = PROVIDER_MANAGER.active().to_provider();
        log::debug!("provider: {:?}", provider);
        let listen_duration = provider.listen_duration();

        cancellable.connect_cancelled(clone!(@weak self as obj => move |_| {
            spawn!(async move {
                obj.imp().audio_recorder.cancel();
                obj.set_state(RecognizerState::Null);
            });
        }));

        if cancellable.is_cancelled()
            || utils::timeout_future(listen_duration, &cancellable.new_child())
                .await
                .is_err()
        {
            return Err(Cancelled::new(&gettext("Cancelled recording")).into());
        }

        let recording = imp.audio_recorder.stop().await.map_err(|err| {
            self.set_state(RecognizerState::Null);
            err
        })?;

        self.set_state(RecognizerState::Recognizing);
        let song = provider.recognize(&recording).await;

        self.set_state(RecognizerState::Null);

        if cancellable.is_cancelled() {
            return Err(Cancelled::new(&gettext("Cancelled recognizing")).into());
        }

        self.emit_by_name::<()>("song-recognized", &[&song?]);

        Ok(())
    }

    fn set_state(&self, state: RecognizerState) {
        if state == self.state() {
            return;
        }

        self.imp().state.set(state);
        self.notify("state");
    }

    fn update_audio_recorder_device_name(&self) {
        let imp = self.imp();

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
    }
}

impl Default for Recognizer {
    fn default() -> Self {
        Self::new()
    }
}

// FIXME handle this inside `audio_recorder`
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
