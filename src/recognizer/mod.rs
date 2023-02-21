mod provider;
mod recorder;
mod recording;

use anyhow::{ensure, Context, Result};
use gst::prelude::*;
use gtk::{
    gio::{self, prelude::*},
    glib::{self, clone, closure_local, subclass::prelude::*, WeakRef},
};

use std::{
    cell::{Cell, Ref, RefCell},
    rc::Rc,
};

pub use self::{
    provider::{ProviderSettings, ProviderType, TestProviderMode},
    recording::RecognizeResult,
};
use self::{recorder::Recorder, recording::Recording};
use crate::{
    audio_device::{self, AudioDeviceClass},
    core::{Cancelled, DateTime},
    model::Song,
    settings::PreferredAudioSource,
    utils,
};

const MAX_SAVED_RECORDING_RECOGNIZE_RETRIES: u8 = 3;

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

    #[derive(Debug, Default, glib::Properties)]
    #[properties(wrapper_type = super::Recognizer)]
    pub struct Recognizer {
        /// Current state
        #[property(get, builder(RecognizerState::default()))]
        pub(super) state: Cell<RecognizerState>,
        /// Whether offline mode is active
        #[property(get)]
        pub(super) is_offline_mode: Cell<bool>,

        pub(super) recorder: Recorder,
        pub(super) cancellable: RefCell<Option<gio::Cancellable>>,

        pub(super) saved_recordings: RefCell<Vec<Rc<Recording>>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Recognizer {
        const NAME: &'static str = "MsaiRecognizer";
        type Type = super::Recognizer;
    }

    impl ObjectImpl for Recognizer {
        crate::derived_properties!();

        fn signals() -> &'static [Signal] {
            static SIGNALS: Lazy<Vec<Signal>> = Lazy::new(|| {
                vec![
                    Signal::builder("recording-peak-changed")
                        .param_types([f64::static_type()])
                        .build(),
                    Signal::builder("song-recognized")
                        .param_types([Song::static_type()])
                        .build(),
                    Signal::builder("recording-saved").build(),
                    Signal::builder("saved-recordings-changed").build(),
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

                    // TODO don't just call when network is available, but also for every
                    // interval if there is network, there are still saved recordings, and
                    // there is currently no recognition in progress.
                    //
                    // This should also be triggered when token is updated.
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
        glib::Object::new()
    }

    pub fn connect_recording_peak_changed<F>(&self, f: F) -> glib::SignalHandlerId
    where
        F: Fn(&Self, f64) + 'static,
    {
        self.connect_closure(
            "recording-peak-changed",
            true,
            closure_local!(|obj: &Self, peak: f64| {
                f(obj, peak);
            }),
        )
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

    pub fn connect_saved_recordings_changed<F>(&self, f: F) -> glib::SignalHandlerId
    where
        F: Fn(&Self) + 'static,
    {
        self.connect_closure(
            "saved-recordings-changed",
            true,
            closure_local!(|obj: &Self| {
                f(obj);
            }),
        )
    }

    pub fn saved_recordings(&self) -> Ref<'_, Vec<Rc<Recording>>> {
        self.imp().saved_recordings.borrow()
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

    async fn recognize(&self, cancellable: &gio::Cancellable) -> Result<()> {
        struct Finally {
            weak: WeakRef<Recognizer>,
        }

        impl Drop for Finally {
            fn drop(&mut self) {
                if let Some(instance) = self.weak.upgrade() {
                    instance.set_state(RecognizerState::Null);
                    let _ = instance.imp().recorder.stop();
                }
            }
        }

        ensure!(
            self.state() == RecognizerState::Null,
            "Recognizer is not in Null state."
        );

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

        let imp = self.imp();

        imp.recorder
            .start(
                Some(&device_name),
                clone!(@weak self as obj => move |peak| {
                    obj.emit_by_name::<()>("recording-peak-changed", &[&peak]);
                }),
            )
            .context("Failed to start recording")?;
        let recorded_time = DateTime::now_local();

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

        let recording_bytes = imp.recorder.stop().context("Failed to stop recording")?;

        if self.is_offline_mode() {
            self.imp()
                .saved_recordings
                .borrow_mut()
                .push(Rc::new(Recording::new(
                    recording_bytes.to_vec(),
                    recorded_time,
                )));
            self.emit_by_name::<()>("saved-recordings-changed", &[]);
            self.emit_by_name::<()>("recording-saved", &[]);
            tracing::debug!("Offline mode is active; saved recording for later recognition");
        } else {
            self.set_state(RecognizerState::Recognizing);

            let song = gio::CancellableFuture::new(
                provider.recognize(&recording_bytes),
                cancellable.clone(),
            )
            .await
            .map_err(|_| Cancelled::new("recognizing while calling provider"))??;
            song.set_last_heard(recorded_time);

            self.emit_by_name::<()>("song-recognized", &[&song]);
        }

        Ok(())
    }

    fn set_state(&self, state: RecognizerState) {
        if state == self.state() {
            return;
        }

        self.imp().state.set(state);
        self.notify_state();
    }

    fn load_saved_recordings(&self) -> Result<()> {
        let recordings: Vec<Rc<Recording>> =
            serde_json::from_str(&utils::app_instance().settings().saved_recordings())?;

        tracing::debug!("Loading {} saved recordings", recordings.len());

        self.imp().saved_recordings.replace(recordings);
        self.emit_by_name::<()>("saved-recordings-changed", &[]);

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

    /// Returned recordings are guaranteed to have a recognizing result.
    /// However, the results may not be successful.
    pub fn take_recognized_saved_recordings(&self) -> Vec<Rc<Recording>> {
        let imp = self.imp();

        let (recognized, to_retain) = imp
            .saved_recordings
            .take()
            .into_iter()
            // FIXME use Vec::drain_filter
            .partition(|recording| is_recording_ready_to_take(recording));
        imp.saved_recordings.replace(to_retain);
        self.emit_by_name::<()>("saved-recordings-changed", &[]);

        recognized
    }

    /// Returned recordings are guaranteed to have a recognizing result.
    /// However, the results may not be successful.
    pub fn peek_recognized_saved_recordings(&self) -> Vec<Rc<Recording>> {
        let imp = self.imp();

        imp.saved_recordings
            .borrow()
            .iter()
            .filter(|recording| is_recording_ready_to_take(recording))
            .cloned()
            .collect()
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

        // TODO recognize recordings concurrently, but not too many at once (at most 3?)
        utils::spawn(clone!(@weak self as obj => async move {
            obj.try_recognize_saved_recordings_inner().await;
        }));
    }

    async fn try_recognize_saved_recordings_inner(&self) {
        let provider = ProviderSettings::lock().active.to_provider();
        tracing::debug!("Recognizing saved recordings with provider: {:?}", provider);

        let saved_recordings_snapshot = self.imp().saved_recordings.borrow().clone();
        for recording in saved_recordings_snapshot {
            if self.is_offline_mode() {
                tracing::debug!("Offline mode is active, cancelled succeeding recognitions");
                break;
            }

            if is_recording_ready_to_take(&recording) {
                tracing::debug!(
                    "Skipping recognition of saved recording: it is already ready to be taken with result: {:?}",
                    recording.recognize_result()
                );
                continue;
            }

            if recording.recognize_retries() > MAX_SAVED_RECORDING_RECOGNIZE_RETRIES {
                tracing::debug!(
                    "Skipping recognition of saved recording: it has already been retried {} times",
                    MAX_SAVED_RECORDING_RECOGNIZE_RETRIES
                );
                continue;
            }

            match provider.recognize(recording.bytes()).await {
                Ok(song) => {
                    song.set_last_heard(recording.recorded_time());

                    recording.set_recognize_result(RecognizeResult::Ok(song));
                    self.emit_by_name::<()>("saved-recordings-changed", &[]);
                }
                Err(err) => {
                    use provider::error::{FingerprintError, NoMatchesError, ResponseParseError};

                    tracing::error!("Failed to recognize saved recording: {:?}", err);

                    recording.increment_recognize_retries();

                    recording.set_recognize_result(RecognizeResult::Err {
                        is_permanent: err.is::<NoMatchesError>()
                            || err.is::<FingerprintError>()
                            || err.is::<ResponseParseError>(),
                        message: err.to_string(),
                    });
                    self.emit_by_name::<()>("saved-recordings-changed", &[]);
                }
            }
        }
    }

    fn update_offline_mode(&self) {
        let is_offline_mode = !gio::NetworkMonitor::default().is_network_available();

        if is_offline_mode == self.is_offline_mode() {
            return;
        }

        self.imp().is_offline_mode.set(is_offline_mode);
        self.notify_is_offline_mode();
    }
}

impl Default for Recognizer {
    fn default() -> Self {
        Self::new()
    }
}

/// Whether the recording is ready to be taken and its result is set and permanent
fn is_recording_ready_to_take(recording: &Recording) -> bool {
    match *recording.recognize_result() {
        None => false,
        Some(RecognizeResult::Ok(_)) => true,
        Some(RecognizeResult::Err { is_permanent, .. }) => is_permanent,
    }
}
