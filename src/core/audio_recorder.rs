use futures_channel::oneshot::{self, Receiver, Sender};
use gst_pbutils::prelude::*;
use gtk::{
    glib::{self, clone},
    subclass::prelude::*,
};

use std::{
    cell::{Cell, RefCell},
    path::Path,
    rc::Rc,
    time::Duration,
};

use super::AudioRecording;

type RecordingItemSender = Rc<RefCell<Option<Sender<anyhow::Result<DoneRecording>>>>>;

#[derive(Debug)]
struct DoneRecording;

#[derive(Debug)]
struct RecordingItem {
    inner: AudioRecording,
    pipeline: gst::Pipeline,
    receiver: RefCell<Option<Receiver<anyhow::Result<DoneRecording>>>>,
}

impl RecordingItem {
    pub fn new(
        path: impl AsRef<Path>,
        device_name: Option<&str>,
        watch_func: impl Fn(&gst::Message, &RecordingItemSender) -> Continue + 'static,
    ) -> anyhow::Result<Self> {
        let (sender, receiver) = oneshot::channel();
        let sender = Rc::new(RefCell::new(Some(sender)));

        let pipeline = default_pipeline(path.as_ref(), device_name)?;
        pipeline
            .bus()
            .unwrap()
            .add_watch_local(
                clone!(@strong sender => @default-return Continue(false), move |_, message| {
                    watch_func(message, &sender)
                }),
            )
            .unwrap();

        Ok(Self {
            inner: AudioRecording::new(path.as_ref()),
            pipeline,
            receiver: RefCell::new(Some(receiver)),
        })
    }

    pub const fn pipeline(&self) -> &gst::Pipeline {
        &self.pipeline
    }

    pub async fn audio_recording(self) -> anyhow::Result<AudioRecording> {
        self.receiver
            .take()
            .ok_or_else(|| anyhow::anyhow!("Dropped receiver"))?
            .await??;
        self.teardown_pipeline()?;
        Ok(self.inner)
    }

    pub fn cleanup(self) {
        if let Err(err) = self.teardown_pipeline() {
            log::warn!("Failed to teardown pipeline during RecordingItem cleanup: {err:?}");
        }
    }

    fn teardown_pipeline(&self) -> anyhow::Result<()> {
        self.pipeline.set_state(gst::State::Null)?;
        let _ = self.pipeline.bus().unwrap().remove_watch();
        Ok(())
    }
}

mod imp {
    use super::*;
    use glib::subclass::Signal;
    use once_cell::sync::Lazy;

    #[derive(Debug, Default)]
    pub struct AudioRecorder {
        pub(super) peak: Cell<f64>,
        pub(super) device_name: RefCell<Option<String>>,

        pub(super) recording_item: RefCell<Option<RecordingItem>>,
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

        fn dispose(&self, _obj: &Self::Type) {
            if let Some(recording_item) = self.recording_item.take() {
                recording_item.cleanup();
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

    pub fn connect_peak_notify<F>(&self, f: F) -> glib::SignalHandlerId
    where
        F: Fn(&Self) + 'static,
    {
        self.connect_notify_local(Some("peak"), move |obj, _| f(obj))
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

    pub fn start(&self, path: impl AsRef<Path>) -> anyhow::Result<()> {
        if let Some(recording_item) = self.imp().recording_item.take() {
            recording_item.cleanup();
        }

        let recording_item = RecordingItem::new(
            path,
            self.device_name().as_deref(),
            clone!(@weak self as obj => @default-return Continue(false), move |message, sender| {
                obj.handle_bus_message(message, sender)
            }),
        )?;

        let imp = self.imp();
        imp.recording_item.replace(Some(recording_item));

        imp.recording_item
            .borrow()
            .as_ref()
            .unwrap()
            .pipeline()
            .set_state(gst::State::Playing)?;

        Ok(())
    }

    pub async fn stop(&self) -> anyhow::Result<AudioRecording> {
        if let Some(recording_item) = self.imp().recording_item.take() {
            log::info!("Sending EOS event to pipeline");
            recording_item.pipeline().send_event(gst::event::Eos::new());
            recording_item.audio_recording().await
        } else {
            Err(anyhow::anyhow!("No pipeline setup"))
        }
    }

    pub fn cancel(&self) {
        if let Some(recording_item) = self.imp().recording_item.take() {
            recording_item.cleanup();
        }

        self.emit_by_name::<()>("stopped", &[]);
    }

    fn handle_bus_message(&self, message: &gst::Message, sender: &RecordingItemSender) -> Continue {
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

                sender.take().unwrap().send(Ok(DoneRecording)).unwrap();
                self.emit_by_name::<()>("stopped", &[]);

                Continue(false)
            }
            MessageView::Error(err) => {
                log::error!(
                    "Error from record bus: {:?} (debug {:#?})",
                    err.error(),
                    err
                );

                sender
                    .take()
                    .unwrap()
                    .send(Err(err.error().into()))
                    .unwrap();
                self.emit_by_name::<()>("stopped", &[]);

                Continue(false)
            }
            MessageView::StateChanged(sc) => {
                if message.src().as_ref()
                    == self
                        .imp()
                        .recording_item
                        .borrow()
                        .as_ref()
                        .map(|recording_item| recording_item.pipeline().upcast_ref::<gst::Object>())
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
