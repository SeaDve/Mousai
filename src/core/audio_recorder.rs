use futures_channel::oneshot::{self, Receiver, Sender};
use gst_pbutils::prelude::*;
use gtk::{
    glib::{self, clone},
    subclass::prelude::*,
};

use std::{
    cell::{Cell, RefCell},
    path::Path,
};

use super::AudioRecording;

mod imp {
    use super::*;
    use once_cell::sync::Lazy;

    #[derive(Debug, Default)]
    pub struct AudioRecorder {
        pub peak: Cell<f64>,

        pub recording: RefCell<Option<AudioRecording>>,
        pub pipeline: RefCell<Option<gst::Pipeline>>,
        pub sender: RefCell<Option<Sender<anyhow::Result<AudioRecording>>>>,
        pub receiver: RefCell<Option<Receiver<anyhow::Result<AudioRecording>>>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for AudioRecorder {
        const NAME: &'static str = "MsaiAudioRecorder";
        type Type = super::AudioRecorder;
        type ParentType = glib::Object;
    }

    impl ObjectImpl for AudioRecorder {
        fn properties() -> &'static [glib::ParamSpec] {
            static PROPERTIES: Lazy<Vec<glib::ParamSpec>> = Lazy::new(|| {
                vec![glib::ParamSpec::new_double(
                    "peak",
                    "Peak",
                    "Current volume peak while recording",
                    f64::MIN,
                    f64::MAX,
                    0.0,
                    glib::ParamFlags::READABLE,
                )]
            });
            PROPERTIES.as_ref()
        }

        fn property(&self, obj: &Self::Type, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
            match pspec.name() {
                "peak" => obj.peak().to_value(),
                _ => unimplemented!(),
            }
        }
    }
}

glib::wrapper! {
    pub struct AudioRecorder(ObjectSubclass<imp::AudioRecorder>);
}

impl AudioRecorder {
    pub fn new() -> Self {
        glib::Object::new::<Self>(&[]).expect("Failed to create AudioRecorder.")
    }

    pub fn connect_peak_notify<F>(&self, f: F) -> glib::SignalHandlerId
    where
        F: Fn(&Self) + 'static,
    {
        self.connect_notify_local(Some("peak"), move |obj, _| f(obj))
    }

    pub fn peak(&self) -> f64 {
        let imp = imp::AudioRecorder::from_instance(self);
        imp.peak.get()
    }

    pub fn start(&self, path: impl AsRef<Path>) -> anyhow::Result<()> {
        let new_recording = AudioRecording::new(path.as_ref());
        let pipeline = Self::default_pipeline(&new_recording.path())?;

        let bus = pipeline.bus().unwrap();
        bus.add_watch_local(
            clone!(@weak self as obj => @default-return Continue(false), move |_, message| {
                obj.handle_bus_message(message)
            }),
        )
        .unwrap();

        let imp = imp::AudioRecorder::from_instance(self);
        imp.pipeline.replace(Some(pipeline));
        imp.recording.replace(Some(new_recording));

        let (sender, receiver) = oneshot::channel();
        imp.sender.replace(Some(sender));
        imp.receiver.replace(Some(receiver));

        let pipeline = imp.pipeline.borrow();
        let pipeline = pipeline.as_ref().unwrap();

        pipeline.set_state(gst::State::Playing)?;

        Ok(())
    }

    pub async fn stop(&self) -> anyhow::Result<AudioRecording> {
        log::info!("Sending EOS event to pipeline");
        self.pipeline()
            .expect("Pipeline not setup")
            .send_event(gst::event::Eos::new());

        let imp = imp::AudioRecorder::from_instance(self);
        let receiver = imp.receiver.take().unwrap();
        receiver.await.unwrap()
    }

    pub async fn cancel(&self) {
        let imp = imp::AudioRecorder::from_instance(self);
        imp.sender.replace(None);
        imp.receiver.replace(None);

        if let Some(recording) = self.cleanup_and_take_recording() {
            if let Err(err) = recording.delete().await {
                log::warn!("Failed to delete recording: {:?}", err);
            }
        }

        log::info!("Cancelled recording");
    }

    pub fn state(&self) -> gst::State {
        self.pipeline().map_or(gst::State::Null, |pipeline| {
            let (_ret, current, _pending) = pipeline.state(None);
            current
        })
    }

    fn pipeline(&self) -> Option<gst::Pipeline> {
        let imp = imp::AudioRecorder::from_instance(self);
        imp.pipeline.borrow().as_ref().cloned()
    }

    fn default_audio_source_name() -> anyhow::Result<String> {
        let server_info = pulsectl::controllers::SourceController::create()?.get_server_info()?;

        server_info
            .default_source_name
            .ok_or_else(|| anyhow::anyhow!("Default audio source name not found"))
    }

    fn default_encodebin_profile() -> gst_pbutils::EncodingContainerProfile {
        let encoding_profile = gst_pbutils::EncodingAudioProfileBuilder::new()
            .format(&gst::Caps::builder("audio/x-opus").build())
            .presence(1)
            .build()
            .unwrap();

        gst_pbutils::EncodingContainerProfileBuilder::new()
            .format(&gst::Caps::builder("application/ogg").build())
            .add_profile(&encoding_profile)
            .build()
            .unwrap()
    }

    fn default_pipeline(recording_path: &Path) -> anyhow::Result<gst::Pipeline> {
        let pipeline = gst::Pipeline::new(None);

        let pulsesrc = gst::ElementFactory::make("pulsesrc", None)?;
        let audioconvert = gst::ElementFactory::make("audioconvert", None)?;
        let level = gst::ElementFactory::make("level", None)?;
        let encodebin = gst::ElementFactory::make("encodebin", None)?;
        let filesink = gst::ElementFactory::make("filesink", None)?;

        match Self::default_audio_source_name() {
            Ok(ref audio_source_name) => {
                log::info!(
                    "Pipeline setup with pulsesrc device name `{}`",
                    audio_source_name
                );
                pulsesrc.set_property("device", audio_source_name)?;
            }
            Err(err) => log::warn!("Failed to set pulsesrc device: {:?}", err),
        }

        encodebin.set_property("profile", &Self::default_encodebin_profile())?;
        filesink.set_property("location", recording_path.to_str().unwrap())?;

        let elements = [&pulsesrc, &audioconvert, &level, &encodebin, &filesink];
        pipeline.add_many(&elements)?;

        pulsesrc.link(&audioconvert)?;
        audioconvert.link_filtered(&level, &gst::Caps::builder("audio/x-raw").build())?;
        level.link(&encodebin)?;
        encodebin.link(&filesink)?;

        for e in elements {
            e.sync_state_with_parent()?;
        }

        Ok(pipeline)
    }

    fn cleanup_and_take_recording(&self) -> Option<AudioRecording> {
        let imp = imp::AudioRecorder::from_instance(self);

        if let Some(pipeline) = imp.pipeline.take() {
            pipeline.set_state(gst::State::Null).unwrap();

            let bus = pipeline.bus().unwrap();
            bus.remove_watch().unwrap();
        }

        imp.recording.take()
    }

    fn handle_bus_message(&self, message: &gst::Message) -> Continue {
        use gst::MessageView;

        match message.view() {
            MessageView::Element(_) => {
                let peak = message
                    .structure()
                    .unwrap()
                    .value("peak")
                    .unwrap()
                    .get::<glib::ValueArray>()
                    .unwrap()
                    .nth(0)
                    .unwrap()
                    .get::<f64>()
                    .unwrap();

                let imp = imp::AudioRecorder::from_instance(self);
                imp.peak.set(peak);
                self.notify("peak");

                Continue(true)
            }
            MessageView::Eos(_) => {
                log::info!("Eos signal received from record bus");

                let recording = self.cleanup_and_take_recording();

                let imp = imp::AudioRecorder::from_instance(self);
                let sender = imp.sender.take().unwrap();
                sender.send(Ok(recording.unwrap())).unwrap();

                Continue(false)
            }
            MessageView::Error(err) => {
                log::error!("Error from record bus: {:?} (debug {:?})", err.error(), err);

                let _recording = self.cleanup_and_take_recording();

                let imp = imp::AudioRecorder::from_instance(self);
                let sender = imp.sender.take().unwrap();
                sender.send(Err(err.error().into())).unwrap();

                Continue(false)
            }
            MessageView::StateChanged(sc) => {
                if message.src().as_ref()
                    == Some(
                        self.pipeline()
                            .expect("Pipeline not setup")
                            .upcast_ref::<gst::Object>(),
                    )
                {
                    log::info!(
                        "Pipeline state set from `{:?}` -> `{:?}`",
                        sc.old(),
                        sc.current()
                    );
                }
                Continue(true)
            }
            _ => Continue(true),
        }
    }
}

impl Default for AudioRecorder {
    fn default() -> Self {
        Self::new()
    }
}
