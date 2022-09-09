mod provider;

use anyhow::{Context, Result};
use futures_util::future::{AbortHandle, Abortable};
use gst::prelude::*;
use gtk::glib::{self, clone, closure_local, subclass::prelude::*};

use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

pub use self::provider::{ProviderSettings, ProviderType, TestProviderMode};
use crate::{
    audio_device::{self, AudioDeviceClass},
    audio_recording::AudioRecording,
    core::{Cancellable, Cancelled},
    model::Song,
    settings::PreferredAudioSource,
    Application,
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

        pub(super) cancellable: RefCell<Option<Rc<Cancellable>>>,
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
                    glib::ParamSpecEnum::builder("state", RecognizerState::static_type())
                        .default_value(RecognizerState::default() as i32)
                        .flags(glib::ParamFlags::READABLE)
                        .build(),
                    // Active recording
                    glib::ParamSpecObject::builder("recording", AudioRecording::static_type())
                        .flags(glib::ParamFlags::READABLE)
                        .build(),
                ]
            });

            PROPERTIES.as_ref()
        }

        fn property(&self, obj: &Self::Type, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
            match pspec.name() {
                "state" => obj.state().to_value(),
                "recording" => obj.recording().to_value(),
                _ => unimplemented!(),
            }
        }

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
    }
}

glib::wrapper! {
    pub struct Recognizer(ObjectSubclass<imp::Recognizer>);
}

impl Recognizer {
    pub fn new() -> Self {
        glib::Object::new(&[]).expect("Failed to create Recognizer.")
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
                let cancellable = Rc::new(Cancellable::default());
                imp.cancellable.replace(Some(Rc::clone(&cancellable)));

                if let Err(err) = self.recognize(&cancellable).await {
                    if let Some(cancelled) = err.downcast_ref::<Cancelled>() {
                        tracing::debug!("Cancelled recognizing: {}", cancelled);
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

    async fn recognize(&self, cancellable: &Cancellable) -> Result<()> {
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
                self.instance.set_state(RecognizerState::Null);
                self.instance.set_recording(None);
            }
        }

        if self.state() != RecognizerState::Null {
            return Err(Cancelled::new("Recognizer is not on null state").into());
        }

        let recording = AudioRecording::new();
        self.set_recording(Some(recording.clone()));

        let _guard = Rc::new(RefCell::new(Some(Guard::new(self))));

        self.set_state(RecognizerState::Listening);

        let device_name = audio_device::find_default_name(
            match Application::default().settings().preferred_audio_source() {
                PreferredAudioSource::Microphone => AudioDeviceClass::Source,
                PreferredAudioSource::DesktopAudio => AudioDeviceClass::Sink,
            },
        )
        .await
        .context("Failed to find default device name")?;
        recording
            .start(Some(&device_name))
            .context("Failed to start recording")?;

        if cancellable.is_cancelled() {
            return Err(Cancelled::new("Stopped while starting to record").into());
        }

        let (recording_timer_handle, recording_timer_abort_reg) = AbortHandle::new_pair();

        cancellable.connect_cancelled(clone!(@weak self as obj, @weak _guard => move |_| {
            recording_timer_handle.abort();
            let _ = _guard.take();
        }));

        let provider = ProviderSettings::lock().active.to_provider();
        tracing::debug!(?provider);

        if Abortable::new(
            glib::timeout_future(provider.listen_duration()),
            recording_timer_abort_reg,
        )
        .await
        .is_err()
        {
            return Err(Cancelled::new("Stopped while listening").into());
        }

        recording.stop().context("Failed to stop recording")?;

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
