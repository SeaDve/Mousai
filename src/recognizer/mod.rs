mod provider;

use gst::prelude::*;
use gtk::glib::{self, clone, closure_local, subclass::prelude::*};

use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

pub use self::provider::{ProviderManager, ProviderType, TestProviderMode};
use crate::{
    core::{AudioRecorder, Cancellable, Cancelled},
    model::Song,
    utils, Application,
};

#[derive(Debug, Default, Clone, Copy, glib::Enum, PartialEq)]
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
        pub(super) state: Cell<RecognizerState>,

        pub(super) audio_recorder: AudioRecorder,
        pub(super) cancellable: RefCell<Option<Cancellable>>,
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
        glib::Object::new(&[]).expect("Failed to create Recognizer.")
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
                        log::info!("Cancelled recognizing: {}", cancelled);
                    } else {
                        return Err(err);
                    }
                }
            }
        }

        Ok(())
    }

    async fn recognize(&self, cancellable: &Cancellable) -> anyhow::Result<()> {
        struct Guard {
            instance: Recognizer,
        }

        impl Guard {
            fn new(recognizer: &Recognizer) -> Guard {
                Guard {
                    instance: recognizer.clone(),
                }
            }
        }

        impl Drop for Guard {
            fn drop(&mut self) {
                self.instance.imp().audio_recorder.cancel();
                self.instance.set_state(RecognizerState::Null);
            }
        }

        if self.state() != RecognizerState::Null {
            return Err(Cancelled::new("Recognizer is not on null state").into());
        }

        let imp = self.imp();

        imp.audio_recorder.set_device_class(
            Application::default()
                .settings()
                .preferred_audio_source()
                .into(),
        );

        let _guard = Rc::new(RefCell::new(Some(Guard::new(self))));

        self.set_state(RecognizerState::Listening);
        imp.audio_recorder.start().await?;

        if cancellable.is_cancelled() {
            return Err(Cancelled::new("Stopped while starting to record").into());
        }

        cancellable.connect_cancelled(clone!(@weak self as obj, @weak _guard => move |_| {
            let _ = _guard.take();
        }));

        let provider = ProviderManager::lock().active.to_provider();
        log::debug!("provider: {:?}", provider);

        if cancellable.is_cancelled()
            || utils::timeout_future(provider.listen_duration(), cancellable)
                .await
                .is_err()
        {
            return Err(Cancelled::new("Stopped while recording").into());
        }

        let recording = imp.audio_recorder.stop().await?;

        if cancellable.is_cancelled() {
            return Err(Cancelled::new("Stopped while flushing the recording").into());
        }

        self.set_state(RecognizerState::Recognizing);
        let song = provider.recognize(&recording).await;

        if cancellable.is_cancelled() {
            return Err(Cancelled::new("Stopped while recognizing").into());
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
