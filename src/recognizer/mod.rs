mod provider;

use gettextrs::gettext;
use gst::prelude::*;
use gtk::{
    glib::{self, clone, closure_local},
    prelude::*,
    subclass::prelude::*,
};

use std::cell::{Cell, RefCell};

pub use self::provider::{ProviderType, TestProviderMode, PROVIDER_MANAGER};
use crate::{
    core::{AudioRecorder, Cancellable, Cancelled},
    model::Song,
    utils, Application,
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
        self.connect_closure(
            "song-recognized",
            true,
            closure_local!(|obj: &Self, song: &Song| {
                f(obj, song);
            }),
        )
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

        if let Err(err) = imp.audio_recorder.start() {
            self.set_state(RecognizerState::Null);
            return Err(err);
        }

        self.set_state(RecognizerState::Listening);
        let provider = PROVIDER_MANAGER.active().to_provider();
        log::debug!("provider: {:?}", provider);
        let listen_duration = provider.listen_duration();

        cancellable.connect_cancelled(clone!(@weak self as obj => move |_| {
            obj.imp().audio_recorder.cancel();
            obj.set_state(RecognizerState::Null);
        }));

        if cancellable.is_cancelled()
            || utils::timeout_future(listen_duration, cancellable)
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
    let preferred_device_class = {
        match Application::default()
            .settings()
            .string("preferred-audio-source")
            .as_str()
        {
            "microphone" => "Audio/Source",
            "desktop-audio" => "Audio/Sink",
            unknown_device_name => anyhow::bail!(
                "Found invalid key `{unknown_device_name}` on `preferred-audio-source`"
            ),
        }
    };

    let device_monitor = gst::DeviceMonitor::new();
    device_monitor.add_filter(Some("Audio/Source"), None);
    device_monitor.add_filter(Some("Audio/Sink"), None);
    device_monitor.start()?;

    log::info!("Finding device name for class `{preferred_device_class}`");

    for device in device_monitor.devices() {
        let device_class = device.device_class();

        if device_class == preferred_device_class {
            let properties = device
                .properties()
                .ok_or_else(|| anyhow::anyhow!("Found no property for device"))?;

            if properties.get::<bool>("is-default")? {
                device_monitor.stop();

                let mut node_name = properties.get::<String>("node.name")?;

                // FIXME test this with actual mic
                if device_class == "Audio/Sink" {
                    node_name.push_str(".monitor");
                }

                return Ok(node_name);
            }
        }
    }

    device_monitor.stop();
    Err(anyhow::anyhow!(
        "Failed to found audio device for class `{preferred_device_class}`"
    ))
}
