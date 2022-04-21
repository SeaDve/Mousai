use gst_pbutils::prelude::*;
use gtk::{
    gio::{self, prelude::*},
    glib::{self, clone, subclass::prelude::*},
};

use std::{
    cell::{Cell, RefCell},
    time::Duration,
};

use super::AudioRecording;

mod imp {
    use super::*;
    use glib::subclass::Signal;
    use once_cell::sync::Lazy;

    #[derive(Debug, Default)]
    pub struct AudioRecorder {
        pub(super) peak: Cell<f64>,
        pub(super) device_name: RefCell<Option<String>>,

        pub(super) pipeline: RefCell<Option<gst::Pipeline>>,
        pub(super) stream: RefCell<Option<gio::MemoryOutputStream>>,
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
            obj.cancel();
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
        if device_name == self.device_name().as_deref() {
            return;
        }

        self.imp()
            .device_name
            .replace(device_name.map(|device_name| device_name.to_string()));
        self.notify("device-name");
    }

    pub fn start(&self) -> anyhow::Result<()> {
        let imp = self.imp();

        if imp.pipeline.borrow().is_some() || imp.stream.borrow().is_some() {
            self.cancel();
        }

        let stream = gio::MemoryOutputStream::new_resizable();
        let pipeline = default_pipeline(&stream, self.device_name().as_deref())?;

        pipeline
            .bus()
            .unwrap()
            .add_watch_local(
                clone!(@weak self as obj => @default-return Continue(false), move |_, message|  {
                    obj.handle_bus_message(message)
                }),
            )
            .unwrap();
        pipeline.set_state(gst::State::Playing)?;

        imp.stream.replace(Some(stream));
        imp.pipeline.replace(Some(pipeline));
        Ok(())
    }

    pub async fn stop(&self) -> anyhow::Result<AudioRecording> {
        let imp = self.imp();

        let pipeline = imp
            .pipeline
            .take()
            .ok_or_else(|| anyhow::anyhow!("No pipeline found"))?;
        pipeline.set_state(gst::State::Null)?;

        let stream = imp
            .stream
            .take()
            .ok_or_else(|| anyhow::anyhow!("No stream found"))?;
        stream.close_future(glib::PRIORITY_HIGH).await?;

        self.emit_by_name::<()>("stopped", &[]);
        let _ = pipeline.bus().unwrap().remove_watch();

        Ok(stream.steal_as_bytes().into())
    }

    pub fn cancel(&self) {
        if let Err(err) = self.cancel_inner() {
            log::warn!("Failed to cancel recording: {err:?}");
        }
    }

    fn cancel_inner(&self) -> anyhow::Result<()> {
        let imp = self.imp();

        let pipeline = imp
            .pipeline
            .take()
            .ok_or_else(|| anyhow::anyhow!("No pipeline found"))?;
        pipeline.set_state(gst::State::Null)?;

        let stream = imp
            .stream
            .take()
            .ok_or_else(|| anyhow::anyhow!("No stream found"))?;
        stream.close(gio::Cancellable::NONE)?;

        self.emit_by_name::<()>("stopped", &[]);
        let _ = pipeline.bus().unwrap().remove_watch();

        Ok(())
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
                Continue(false)
            }
            MessageView::Error(err) => {
                log::error!(
                    "Error from record bus: {:?} (debug {:#?})",
                    err.error(),
                    err
                );

                self.cancel();

                Continue(false)
            }
            MessageView::StateChanged(sc) => {
                if message.src().as_ref()
                    == self
                        .imp()
                        .pipeline
                        .borrow()
                        .as_ref()
                        .map(|pipeline| pipeline.upcast_ref::<gst::Object>())
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
    stream: &gio::MemoryOutputStream,
    device_name: Option<&str>,
) -> anyhow::Result<gst::Pipeline> {
    let pipeline = gst::Pipeline::new(None);

    let pulsesrc = gst::ElementFactory::make("pulsesrc", None)?;
    let audioconvert = gst::ElementFactory::make("audioconvert", None)?;
    let level = gst::ElementFactory::make("level", None)?;
    let encodebin = gst::ElementFactory::make("encodebin", None)?;
    let giostreamsink = gst::ElementFactory::make("giostreamsink", None)?;

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
    giostreamsink.set_property("stream", stream);

    let elements = [&pulsesrc, &audioconvert, &level, &encodebin, &giostreamsink];
    pipeline.add_many(&elements)?;

    pulsesrc.link(&audioconvert)?;
    audioconvert.link_filtered(&level, &gst::Caps::builder("audio/x-raw").build())?;
    level.link(&encodebin)?;
    encodebin.link(&giostreamsink)?;

    for e in elements {
        e.sync_state_with_parent()?;
    }

    Ok(pipeline)
}
