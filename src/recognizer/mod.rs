mod provider;
mod recorder;
mod recording;
mod recordings;

use anyhow::{ensure, Context, Result};
use gettextrs::gettext;
use gst::prelude::*;
use gtk::{
    gio::{self, prelude::*},
    glib::{self, clone, closure_local, subclass::prelude::*, WeakRef},
};
use once_cell::unsync::OnceCell;

use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

pub use self::{
    provider::{
        ProviderSettings, ProviderType, RecognizeError, RecognizeErrorKind, TestProviderMode,
    },
    recordings::Recordings,
};
use self::{
    recorder::Recorder,
    recording::{BoxedRecognizeResult, Recording},
};
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

        pub(super) saved_recordings: OnceCell<Recordings>,
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
                    Signal::builder("recording-saved")
                        .param_types([String::static_type()])
                        .build(),
                ]
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

    fn emit_recording_peak_changed(&self, peak: f64) {
        self.emit_by_name::<()>("recording-peak-changed", &[&peak]);
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

    fn emit_song_recognized(&self, song: &Song) {
        self.emit_by_name::<()>("song-recognized", &[song]);
    }

    pub fn connect_recording_saved<F>(&self, f: F) -> glib::SignalHandlerId
    where
        F: Fn(&Self, &str) + 'static,
    {
        self.connect_closure(
            "recording-saved",
            true,
            closure_local!(|obj: &Self, message: &str| {
                f(obj, message);
            }),
        )
    }

    fn emit_recording_saved(&self, message: &str) {
        self.emit_by_name::<()>("recording-saved", &[&message]);
    }

    pub fn bind_saved_recordings(&self, recordings: &Recordings) {
        self.imp()
            .saved_recordings
            .set(recordings.clone())
            .expect("saved recordings must be bound only once");

        gio::NetworkMonitor::default().connect_network_available_notify(
            clone!(@weak self as obj => move |_| {
                obj.update_offline_mode();

                // TODO don't just call when network is available, but also for every
                // interval if there is network, there are still saved recordings, and
                // there is currently no recognition in progress.
                //
                // This should also be triggered when token is updated.
                obj.try_recognize_saved_recordings();
            }),
        );

        self.update_offline_mode();

        self.try_recognize_saved_recordings();
    }

    pub fn saved_recordings(&self) -> &Recordings {
        self.imp()
            .saved_recordings
            .get()
            .expect("saved recordings must be bound")
    }

    /// Returned recordings are guaranteed to have a recognizing result.
    /// However, the results may not be successful.
    pub fn take_recognized_saved_recordings(&self) -> Result<Vec<Recording>> {
        self.saved_recordings()
            .take_filtered(is_recording_ready_to_take)
    }

    /// Returned recordings are guaranteed to have a recognizing result.
    /// However, the results may not be successful.
    pub fn peek_recognized_saved_recordings(&self) -> Vec<Recording> {
        self.saved_recordings()
            .peek_filtered(is_recording_ready_to_take)
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
                    obj.emit_recording_peak_changed(peak);
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
        tracing::debug!(
            "Stopped recording with size {}",
            glib::format_size_full(
                recording_bytes.len() as u64,
                glib::FormatSizeFlags::LONG_FORMAT
            )
        );

        if self.is_offline_mode() {
            self.saved_recordings()
                .insert(Recording::new(&recording_bytes, &recorded_time))
                .context("Failed to insert recording")?;
            self.emit_recording_saved(&gettext(
                "The result will be available when you're back online.",
            ));
            tracing::debug!("Offline mode is active; saved recording for later recognition");
            return Ok(());
        }

        self.set_state(RecognizerState::Recognizing);

        let res =
            gio::CancellableFuture::new(provider.recognize(&recording_bytes), cancellable.clone())
                .await
                .map_err(|_| Cancelled::new("recognizing while calling provider"))?;

        match res {
            Ok(song) => {
                song.set_last_heard(recorded_time);

                self.emit_song_recognized(&song);
            }
            Err(err) => {
                if err.is_permanent() {
                    return Err(err.into());
                }

                self.saved_recordings()
                    .insert(Recording::new(&recording_bytes, &recorded_time))
                    .context("Failed to insert recording")?;
                let message = match err.kind() {
                    RecognizeErrorKind::Connection => {
                        gettext("The result will be available when your connection is restored.")
                    }
                    RecognizeErrorKind::TokenLimitReached => {
                        gettext("The result will be available when your token limit is reset.")
                    }
                    RecognizeErrorKind::InvalidToken => gettext(
                        "The result will be available when your token is replaced with a valid one.",
                    ),
                    RecognizeErrorKind::NoMatches
                    | RecognizeErrorKind::Fingerprint
                    | RecognizeErrorKind::OtherPermanent => {
                        unreachable!("permanent errors should have been returned")
                    }
                };
                self.emit_recording_saved(&message);
                tracing::debug!("Recognition failed with non-permanent error `{:?}`; saved recording for later recognition", err);
            }
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

    fn try_recognize_saved_recordings(&self) {
        let saved_recordings = self.saved_recordings();

        if saved_recordings.is_empty() {
            return;
        }

        if self.is_offline_mode() {
            tracing::debug!(
                "Offline mode is active, skipping recognition of {} saved recordings",
                saved_recordings.n_items()
            );
            return;
        }

        // TODO recognize recordings concurrently, but not too many at once (at most 3?)
        utils::spawn(
            glib::PRIORITY_DEFAULT,
            clone!(@weak self as obj => async move {
                obj.try_recognize_saved_recordings_inner().await;
            }),
        );
    }

    async fn try_recognize_saved_recordings_inner(&self) {
        let provider = ProviderSettings::lock().active.to_provider();
        tracing::debug!("Recognizing saved recordings with provider: {:?}", provider);

        let saved_recordings_snapshot = self.saved_recordings().peek_filtered(|_| true);
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

            match provider.recognize(recording.bytes().as_ref()).await {
                Ok(song) => {
                    song.set_last_heard(recording.recorded_time());
                    recording.set_recognize_result(Some(BoxedRecognizeResult(Ok(song))));
                }
                Err(err) => {
                    tracing::error!("Failed to recognize saved recording: {:?}", err);
                    recording.increment_recognize_retries();
                    recording.set_recognize_result(Some(BoxedRecognizeResult(Err(err))));
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
    match recording.recognize_result().map(|r| r.0) {
        None => false,
        Some(Ok(_)) => true,
        Some(Err(ref err)) => err.is_permanent(),
    }
}
