mod provider;

use anyhow::{ensure, Context, Result};
use gst::prelude::*;
use gtk::{
    gio::{self, prelude::*},
    glib::{self, clone, closure_local, subclass::prelude::*, WeakRef},
};

use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

pub use self::provider::{ProviderSettings, ProviderType, TestProviderMode};
use crate::{
    audio_device::{self, AudioDeviceClass},
    audio_recording::AudioRecording,
    core::Cancelled,
    model::Song,
    settings::PreferredAudioSource,
    utils,
};

#[derive(Debug, Default, Clone, Copy, glib::Enum, PartialEq, Eq)]
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
        pub(super) recording: RefCell<Option<AudioRecording>>,

        pub(super) cancellable: RefCell<Option<gio::Cancellable>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Recognizer {
        const NAME: &'static str = "MsaiRecognizer";
        type Type = super::Recognizer;
    }

    impl ObjectImpl for Recognizer {
        fn properties() -> &'static [glib::ParamSpec] {
            static PROPERTIES: Lazy<Vec<glib::ParamSpec>> = Lazy::new(|| {
                vec![
                    // Current state of Self
                    glib::ParamSpecEnum::builder("state", RecognizerState::default())
                        .read_only()
                        .build(),
                    // Active recording
                    glib::ParamSpecObject::builder::<AudioRecording>("recording")
                        .read_only()
                        .build(),
                ]
            });

            PROPERTIES.as_ref()
        }

        fn property(&self, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
            let obj = self.obj();

            match pspec.name() {
                "state" => obj.state().to_value(),
                "recording" => obj.recording().to_value(),
                _ => unimplemented!(),
            }
        }

        fn signals() -> &'static [Signal] {
            static SIGNALS: Lazy<Vec<Signal>> = Lazy::new(|| {
                vec![Signal::builder("song-recognized")
                    .param_types([Song::static_type()])
                    .build()]
            });

            SIGNALS.as_ref()
        }
    }
}

glib::wrapper! {
    pub struct Recognizer(ObjectSubclass<imp::Recognizer>);
}

impl Recognizer {
    pub fn new() -> Self {
        glib::Object::new(&[])
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

    pub fn connect_recording_notify<F>(&self, f: F) -> glib::SignalHandlerId
    where
        F: Fn(&Self) + 'static,
    {
        self.connect_notify_local(Some("recording"), move |obj, _| f(obj))
    }

    pub fn recording(&self) -> Option<AudioRecording> {
        self.imp().recording.borrow().clone()
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

    pub async fn toggle_recognize(&self) -> Result<()> {
        let imp = self.imp();

        match self.state() {
            RecognizerState::Listening | RecognizerState::Recognizing => {
                if let Some(cancellable) = imp.cancellable.take() {
                    cancellable.cancel();
                }
            }
            RecognizerState::Null => {
                let cancellable = gio::Cancellable::default();
                imp.cancellable.replace(Some(cancellable.clone()));

                if let Err(err) = self.recognize(&cancellable).await {
                    if let Some(cancelled) = err.downcast_ref::<Cancelled>() {
                        tracing::debug!("{}", cancelled);
                    } else {
                        return Err(err);
                    }
                }
            }
        }

        Ok(())
    }

    fn set_recording(&self, recording: Option<AudioRecording>) {
        if recording == self.recording() {
            return;
        }

        self.imp().recording.replace(recording);
        self.notify("recording");
    }

    async fn recognize(&self, cancellable: &gio::Cancellable) -> Result<()> {
        struct Finally {
            weak: WeakRef<Recognizer>,
        }

        impl Drop for Finally {
            fn drop(&mut self) {
                if let Some(instance) = self.weak.upgrade() {
                    instance.set_state(RecognizerState::Null);
                    instance.set_recording(None);
                }
            }
        }

        ensure!(
            self.state() == RecognizerState::Null,
            "Recognizer is not in Null state."
        );

        let recording = AudioRecording::new();
        self.set_recording(Some(recording.clone()));

        let _finally = Rc::new(RefCell::new(Some(Finally {
            weak: self.downgrade(),
        })));

        self.set_state(RecognizerState::Listening);

        let device_name = gio::CancellableFuture::new(
            audio_device::find_default_name(
                match utils::app_instance().settings().preferred_audio_source() {
                    PreferredAudioSource::Microphone => AudioDeviceClass::Source,
                    PreferredAudioSource::DesktopAudio => AudioDeviceClass::Sink,
                },
            ),
            cancellable.clone(),
        )
        .await
        .map_err(|_| Cancelled::new("recognizing while finding default audio device name"))?
        .context("Failed to find default device name")?;

        recording
            .start(Some(&device_name))
            .context("Failed to start recording")?;

        cancellable.connect_cancelled_local(clone!(@weak _finally => move |_| {
            let _ = _finally.take();
        }));

        let provider = ProviderSettings::lock().active.to_provider();
        tracing::debug!(?provider);

        gio::CancellableFuture::new(
            glib::timeout_future(provider.listen_duration()),
            cancellable.clone(),
        )
        .await
        .map_err(|_| Cancelled::new("recognizing while recording"))?;

        recording.stop().context("Failed to stop recording")?;

        self.set_state(RecognizerState::Recognizing);
        let song = gio::CancellableFuture::new(provider.recognize(&recording), cancellable.clone())
            .await
            .map_err(|_| Cancelled::new("recognizing while calling provider"))?;

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
