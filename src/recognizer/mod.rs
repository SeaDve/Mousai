mod provider;

use gettextrs::gettext;
use gst::prelude::*;
use gtk::{
    glib::{self, clone, closure_local},
    subclass::prelude::*,
};

use std::cell::{Cell, RefCell};

pub use self::provider::{ProviderManager, ProviderType, TestProviderMode};
use crate::{
    core::{AudioRecorder, Cancellable, Cancelled},
    model::Song,
    utils, Application,
};

#[derive(Default, Debug, Clone, Copy, glib::Enum, PartialEq)]
#[enum_type(name = "MsaiRecognizerState")]
pub enum RecognizerState {
    #[default]
    Null,
    Listening,
    Recognizing,
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
                vec![
                    // Current state of Self
                    glib::ParamSpecEnum::builder("state", RecognizerState::static_type())
                        .default_value(RecognizerState::default() as i32)
                        .flags(glib::ParamFlags::READABLE)
                        .build(),
                ]
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
        let imp = self.imp();

        imp.audio_recorder.set_device_class(
            Application::default()
                .settings()
                .preferred_audio_source()
                .into(),
        );

        if let Err(err) = imp.audio_recorder.start().await {
            self.set_state(RecognizerState::Null);
            return Err(err);
        }

        self.set_state(RecognizerState::Listening);
        let provider = ProviderManager::global().active().to_provider();
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
}

impl Default for Recognizer {
    fn default() -> Self {
        Self::new()
    }
}
