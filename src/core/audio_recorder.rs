use futures_channel::oneshot::{self, Receiver, Sender};
use gst_pbutils::prelude::*;
use gtk::{
    glib::{self, clone},
    subclass::prelude::*,
};

use std::{
    cell::{Cell, RefCell},
    path::Path,
    time::Duration,
};

use super::AudioRecording;

mod imp {
    use super::*;
    use glib::subclass::Signal;
    use once_cell::sync::Lazy;

    #[derive(Debug, Default)]
    pub struct AudioRecorder {
        pub peak: Cell<f64>,
        pub device_name: RefCell<Option<String>>,

        pub recording: RefCell<Option<AudioRecording>>,
        pub pipeline: RefCell<Option<gst::Pipeline>>,
        pub sender: RefCell<Option<Sender<anyhow::Result<AudioRecording>>>>,
        pub receiver: RefCell<Option<Receiver<anyhow::Result<AudioRecording>>>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for AudioRecorder {
        const NAME: &'static str = "MsaiAudioRecorder";
        type Type = super::AudioRecorder;
    }

    impl ObjectImpl for AudioRecorder {
        fn signals() -> &'static [Signal] {
            static SIGNALS: Lazy<Vec<Signal>> = Lazy::new(|| {
                vec![Signal::builder("stopped", &[], <()>::static_type().into()).build()]
            });
            SIGNALS.as_ref()
        }

        fn properties() -> &'static [glib::ParamSpec] {
            static PROPERTIES: Lazy<Vec<glib::ParamSpec>> = Lazy::new(|| {
                vec![
                    glib::ParamSpecDouble::new(
                        "peak",
                        "Peak",
                        "Current volume peak while recording",
                        f64::MIN,
                        f64::MAX,
                        0.0,
                        glib::ParamFlags::READABLE,
                    ),
                    glib::ParamSpecString::new(
                        "device-name",
                        "Device Name",
                        "The device name pulsesrc will use",
                        None,
                        glib::ParamFlags::READWRITE | glib::ParamFlags::EXPLICIT_NOTIFY,
                    ),
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
                "device-name" => {
                    let device_name = value.get().unwrap();
                    obj.set_device_name(device_name);
                }
                _ => unimplemented!(),
            }
        }

        fn property(&self, obj: &Self::Type, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
            match pspec.name() {
                "peak" => obj.peak().to_value(),
                "device-name" => obj.device_name().to_value(),
                _ => unimplemented!(),
            }
        }

        fn dispose(&self, obj: &Self::Type) {
            let _recording = obj.cleanup_and_take_recording();
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

    pub fn connect_stopped<F>(&self, f: F) -> glib::SignalHandlerId
    where
        F: Fn(&Self) + 'static,
    {
        self.connect_local("stopped", true, move |values| {
            let obj = values[0].get::<Self>().unwrap();
            f(&obj);
            None
        })
    }

    pub fn peak(&self) -> f64 {
        self.imp().peak.get()
    }

    pub fn device_name(&self) -> Option<String> {
        self.imp().device_name.borrow().clone()
    }

    pub fn set_device_name(&self, device_name: Option<&str>) {
        self.imp()
            .device_name
            .replace(device_name.map(|device_name| device_name.to_string()));

        self.notify("device-name");
    }

    pub fn connect_peak_notify<F>(&self, f: F) -> glib::SignalHandlerId
    where
        F: Fn(&Self) + 'static,
    {
        self.connect_notify_local(Some("peak"), move |obj, _| f(obj))
    }

    pub fn start(&self, path: impl AsRef<Path>) -> anyhow::Result<()> {
        let new_recording = AudioRecording::new(path.as_ref());
        let pipeline = default_pipeline(&new_recording.path(), self.device_name().as_deref())?;

        let bus = pipeline.bus().unwrap();
        bus.add_watch_local(
            clone!(@weak self as obj => @default-return Continue(false), move |_, message| {
                obj.handle_bus_message(message)
            }),
        )
        .unwrap();

        let imp = self.imp();
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

        let receiver = self.imp().receiver.take().unwrap();
        receiver.await.unwrap()
    }

    pub async fn cancel(&self) {
        let imp = self.imp();
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
        self.imp().pipeline.borrow().as_ref().cloned()
    }

    fn cleanup_and_take_recording(&self) -> Option<AudioRecording> {
        let imp = self.imp();

        if let Some(pipeline) = imp.pipeline.take() {
            pipeline.set_state(gst::State::Null).unwrap();

            let bus = pipeline.bus().unwrap();
            bus.remove_watch().unwrap();
        }

        self.emit_by_name::<()>("stopped", &[]);

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

                self.imp().peak.set(peak);
                self.notify("peak");

                Continue(true)
            }
            MessageView::Eos(_) => {
                log::info!("Eos signal received from record bus");

                let recording = self.cleanup_and_take_recording();

                let sender = self.imp().sender.take().unwrap();
                sender.send(Ok(recording.unwrap())).unwrap();

                Continue(false)
            }
            MessageView::Error(err) => {
                log::error!(
                    "Error from record bus: {:?} (debug {:#?})",
                    err.error(),
                    err
                );

                let _recording = self.cleanup_and_take_recording();

                let sender = self.imp().sender.take().unwrap();
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

fn default_pipeline(
    recording_path: &Path,
    device_name: Option<&str>,
) -> anyhow::Result<gst::Pipeline> {
    let pipeline = gst::Pipeline::new(None);

    let pulsesrc = gst::ElementFactory::make("pulsesrc", None)?;
    let audioconvert = gst::ElementFactory::make("audioconvert", None)?;
    let level = gst::ElementFactory::make("level", None)?;
    let encodebin = gst::ElementFactory::make("encodebin", None)?;
    let filesink = gst::ElementFactory::make("filesink", None)?;

    let encodebin_profile = {
        let audio_caps = gst::Caps::new_simple("audio/x-opus", &[]);
        let encoding_profile = gst_pbutils::EncodingAudioProfile::builder(&audio_caps)
            .presence(1)
            .build();

        let container_caps = gst::Caps::new_simple("application/ogg", &[]);
        gst_pbutils::EncodingContainerProfile::builder(&container_caps)
            .add_profile(&encoding_profile)
            .build()
    };

    pulsesrc.set_property("device", device_name);
    level.set_property("interval", Duration::from_millis(80).as_nanos() as u64);
    level.set_property("peak-ttl", Duration::from_millis(80).as_nanos() as u64);
    encodebin.set_property("profile", encodebin_profile);
    filesink.set_property("location", recording_path.to_str().unwrap());

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
