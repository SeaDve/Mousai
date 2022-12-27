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

#[derive(Debug, Clone, glib::Boxed)]
#[boxed_type(name = "MsaiBoxedSongVec")]
struct BoxedSongVec(Vec<Song>);

mod imp {
    use super::*;
    use glib::subclass::Signal;
    use once_cell::sync::Lazy;

    #[derive(Debug, Default)]
    pub struct Recognizer {
        pub(super) state: Cell<RecognizerState>,
        pub(super) recording: RefCell<Option<AudioRecording>>,
        pub(super) is_offline_mode: Cell<bool>,

        pub(super) saved_recordings: RefCell<Vec<AudioRecording>>,
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
                    glib::ParamSpecBoolean::builder("offline-mode")
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
                "offline-mode" => obj.is_offline_mode().to_value(),
                _ => unimplemented!(),
            }
        }

        fn signals() -> &'static [Signal] {
            static SIGNALS: Lazy<Vec<Signal>> = Lazy::new(|| {
                vec![
                    Signal::builder("song-recognized")
                        .param_types([Song::static_type()])
                        .build(),
                    Signal::builder("saved-songs-recognized")
                        .param_types([BoxedSongVec::static_type()])
                        .build(),
                    Signal::builder("recording-saved").build(),
                ]
            });

            SIGNALS.as_ref()
        }

        fn constructed(&self) {
            self.parent_constructed();

            let obj = self.obj();

            // TODO Handle outside and improve timings
            if let Err(err) = obj.load_saved_recordings() {
                tracing::error!("Failed to load saved recordings: {:?}", err);
            }

            gio::NetworkMonitor::default().connect_network_available_notify(
                clone!(@weak obj => move |_| {
                    obj.update_offline_mode();
                    obj.try_recognize_saved_recordings();
                }),
            );

            obj.update_offline_mode();
            obj.try_recognize_saved_recordings();
        }
    }
}

glib::wrapper! {
    pub struct Recognizer(ObjectSubclass<imp::Recognizer>);
}

impl Recognizer {
    pub fn new() -> Self {
        glib::Object::builder().build()
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

    pub fn connect_offline_mode_notify<F>(&self, f: F) -> glib::SignalHandlerId
    where
        F: Fn(&Self) + 'static,
    {
        self.connect_notify_local(Some("offline-mode"), move |obj, _| f(obj))
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

    pub fn connect_saved_songs_recognized<F>(&self, f: F) -> glib::SignalHandlerId
    where
        F: Fn(&Self, &[Song]) + 'static,
    {
        self.connect_closure(
            "saved-songs-recognized",
            true,
            closure_local!(|obj: &Self, boxed_song_vec: BoxedSongVec| {
                f(obj, &boxed_song_vec.0);
            }),
        )
    }

    pub fn connect_recording_saved<F>(&self, f: F) -> glib::SignalHandlerId
    where
        F: Fn(&Self) + 'static,
    {
        self.connect_closure(
            "recording-saved",
            true,
            closure_local!(|obj: &Self| {
                f(obj);
            }),
        )
    }

    pub fn is_offline_mode(&self) -> bool {
        self.imp().is_offline_mode.get()
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

        if self.is_offline_mode() {
            self.imp().saved_recordings.borrow_mut().push(recording);
            self.emit_by_name::<()>("recording-saved", &[]);
            tracing::debug!("Offline mode is active; saved recording for later recognition");
        } else {
            self.set_state(RecognizerState::Recognizing);

            let song =
                gio::CancellableFuture::new(provider.recognize(&recording), cancellable.clone())
                    .await
                    .map_err(|_| Cancelled::new("recognizing while calling provider"))??;
            song.set_is_newly_recognized(true);
            song.set_last_heard(
                recording
                    .recorded_time()
                    .expect("Recording must have a time when started")
                    .clone(),
            );

            self.emit_by_name::<()>("song-recognized", &[&song]);
        }

        Ok(())
    }

    fn set_state(&self, state: RecognizerState) {
        if state == self.state() {
            return;
        }

        self.imp().state.set(state);
        self.notify("state");
    }

    fn load_saved_recordings(&self) -> Result<()> {
        let recordings: Vec<AudioRecording> =
            serde_json::from_str(&utils::app_instance().settings().saved_recordings())?;

        tracing::debug!("Loading {} saved recordings", recordings.len());

        self.imp().saved_recordings.replace(recordings);

        Ok(())
    }

    pub fn save_saved_recordings(&self) -> Result<()> {
        let saved_recordings = self.imp().saved_recordings.borrow();

        tracing::debug!("Saving {} saved recordings", saved_recordings.len());

        utils::app_instance()
            .settings()
            .set_saved_recordings(&serde_json::to_string(saved_recordings.as_slice())?);

        Ok(())
    }

    fn try_recognize_saved_recordings(&self) {
        let saved_recordings = self.imp().saved_recordings.borrow();

        if saved_recordings.is_empty() {
            return;
        }

        if self.is_offline_mode() {
            tracing::debug!(
                "Offline mode is active, skipping recognition of {} saved recordings",
                saved_recordings.len()
            );
            return;
        }

        let provider = ProviderSettings::lock().active.to_provider();
        tracing::debug!("Recognizing saved recordings with provider: {:?}", provider);

        // TODO recognize recordings concurrently
        utils::spawn(clone!(@weak self as obj => async move {
            let imp = obj.imp();

            let mut recognized = Vec::new();
            let mut to_return = Vec::new();

            for audio in imp.saved_recordings.take() {
                match provider.recognize(&audio).await {
                    Ok(song) => {
                        song.set_is_newly_recognized(true);
                        song.set_last_heard(
                            audio
                                .recorded_time()
                                .expect("Recording must have a time when started")
                                .clone(),
                        );
                        recognized.push(song);
                    }
                    Err(err) => {
                        // TODO don't return no match errors
                        to_return.push(audio);
                        // TODO propagate error
                        tracing::error!("Failed to recognize saved recording: {:?}", err);
                    }
                }
            }

            tracing::debug!("Failed to recognize {} saved recordings and was returned", to_return.len());
            imp.saved_recordings.replace(to_return);

            // TODO Consider showing a notification if some recordings were not recognized
            if !recognized.is_empty() {
                tracing::debug!("Successfully recognized {} saved recordings", recognized.len());
                obj.emit_by_name::<()>("saved-songs-recognized", &[&BoxedSongVec(recognized)]);
            }
        }));
    }

    fn update_offline_mode(&self) {
        let is_offline_mode = !gio::NetworkMonitor::default().is_network_available();

        if is_offline_mode == self.is_offline_mode() {
            return;
        }

        self.imp().is_offline_mode.set(is_offline_mode);
        self.notify("offline-mode");
    }
}

impl Default for Recognizer {
    fn default() -> Self {
        Self::new()
    }
}
