use anyhow::Context;
use gst::prelude::*;
use gtk::{
    gio::{self, prelude::*},
    glib::{self, clone, closure_local, subclass::prelude::*},
};

use std::{
    cell::{Cell, RefCell},
    time::Duration,
};

use super::{AudioDeviceClass, AudioRecording};
use crate::THREAD_POOL;

mod imp {
    use super::*;
    use glib::subclass::Signal;
    use once_cell::sync::Lazy;

    #[derive(Debug, Default)]
    pub struct AudioRecorder {
        pub(super) device_class: Cell<AudioDeviceClass>,

        pub(super) current: RefCell<Option<(gst::Pipeline, gio::MemoryOutputStream)>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for AudioRecorder {
        const NAME: &'static str = "MsaiAudioRecorder";
        type Type = super::AudioRecorder;
    }

    impl ObjectImpl for AudioRecorder {
        fn signals() -> &'static [Signal] {
            static SIGNALS: Lazy<Vec<Signal>> = Lazy::new(|| {
                vec![
                    Signal::builder("stopped", &[], <()>::static_type().into()).build(),
                    Signal::builder(
                        "peak",
                        &[f64::static_type().into()],
                        <()>::static_type().into(),
                    )
                    .build(),
                ]
            });

            SIGNALS.as_ref()
        }

        fn properties() -> &'static [glib::ParamSpec] {
            static PROPERTIES: Lazy<Vec<glib::ParamSpec>> = Lazy::new(|| {
                vec![
                    // The device class to look for
                    glib::ParamSpecEnum::builder("device-class", AudioDeviceClass::static_type())
                        .default_value(AudioDeviceClass::default() as i32)
                        .flags(glib::ParamFlags::READWRITE | glib::ParamFlags::EXPLICIT_NOTIFY)
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
                "device-class" => {
                    let device_class = value.get().unwrap();
                    obj.set_device_class(device_class);
                }
                _ => unimplemented!(),
            }
        }

        fn property(&self, obj: &Self::Type, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
            match pspec.name() {
                "device-class" => obj.device_class().to_value(),
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
        glib::Object::new(&[]).expect("Failed to create AudioRecorder.")
    }

    pub fn connect_stopped<F>(&self, f: F) -> glib::SignalHandlerId
    where
        F: Fn(&Self) + 'static,
    {
        self.connect_closure(
            "stopped",
            true,
            closure_local!(|obj: &Self| {
                f(obj);
            }),
        )
    }

    pub fn connect_peak<F>(&self, f: F) -> glib::SignalHandlerId
    where
        F: Fn(&Self, f64) + 'static,
    {
        self.connect_closure(
            "peak",
            true,
            closure_local!(|obj: &Self, peak: f64| {
                f(obj, peak);
            }),
        )
    }

    pub fn device_class(&self) -> AudioDeviceClass {
        self.imp().device_class.get()
    }

    pub fn set_device_class(&self, device_class: AudioDeviceClass) {
        if device_class == self.device_class() {
            return;
        }

        self.imp().device_class.set(device_class);
        self.notify("device-class");
    }

    pub async fn start(&self) -> anyhow::Result<()> {
        let imp = self.imp();

        if imp.current.borrow().is_some() {
            log::warn!("Tried to start another recording without stopping existing one");
            self.cancel();
        }

        let stream = gio::MemoryOutputStream::new_resizable();
        let pipeline = create_pipeline(&stream, self.device_class()).await?;

        if imp.current.borrow().is_some() {
            log::warn!(
                "Another recording was started while another was awaiting during pipeline creation"
            );
            self.cancel();
        }

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

        imp.current.replace(Some((pipeline, stream)));
        Ok(())
    }

    pub async fn stop(&self) -> anyhow::Result<AudioRecording> {
        let imp = self.imp();

        let (pipeline, stream) = imp
            .current
            .take()
            .ok_or_else(|| anyhow::anyhow!("No current recording found"))?;

        pipeline.set_state(gst::State::Null)?;
        stream.close_future(glib::PRIORITY_HIGH).await?;

        self.emit_by_name::<()>("stopped", &[]);
        let _ = pipeline.bus().unwrap().remove_watch();

        let bytes = stream.steal_as_bytes();
        log::info!(
            "Recorded audio with size {}",
            glib::format_size(bytes.len() as u64)
        );

        Ok(bytes.into())
    }

    pub fn cancel(&self) {
        if let Err(err) = self.cancel_inner() {
            log::warn!("Failed to cancel recording: {err:?}");
        }
    }

    fn cancel_inner(&self) -> anyhow::Result<()> {
        let imp = self.imp();

        let (pipeline, stream) = imp
            .current
            .take()
            .ok_or_else(|| anyhow::anyhow!("No current recording found"))?;

        pipeline.set_state(gst::State::Null)?;
        stream.close(gio::Cancellable::NONE)?;

        self.emit_by_name::<()>("stopped", &[]);
        let _ = pipeline.bus().unwrap().remove_watch();

        Ok(())
    }

    fn handle_bus_message(&self, message: &gst::Message) -> Continue {
        use gst::MessageView;

        match message.view() {
            MessageView::Element(element) => {
                if let Some(structure) = element.structure() {
                    if structure.has_name("level") {
                        let peak = structure
                            .get::<&glib::ValueArray>("peak")
                            .unwrap()
                            .nth(0)
                            .unwrap()
                            .get::<f64>()
                            .unwrap();
                        let normalized_peak = 10_f64.powf(peak / 20.0);
                        self.emit_by_name::<()>("peak", &[&normalized_peak]);
                    }
                }

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
                        .current
                        .borrow()
                        .as_ref()
                        .map(|(pipeline, _)| pipeline.upcast_ref::<gst::Object>())
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

fn find_default_device_name(preferred_device_class: AudioDeviceClass) -> anyhow::Result<String> {
    let device_monitor = gst::DeviceMonitor::new();
    device_monitor.add_filter(Some(AudioDeviceClass::Source.as_str()), None);
    device_monitor.add_filter(Some(AudioDeviceClass::Sink.as_str()), None);

    device_monitor.start()?;
    let devices = device_monitor.devices();
    device_monitor.stop();

    log::info!("Finding device name for class `{preferred_device_class:?}`");

    for device in devices {
        let device_class = AudioDeviceClass::for_str(&device.device_class())?;

        if device_class == preferred_device_class {
            let properties = device
                .properties()
                .ok_or_else(|| anyhow::anyhow!("No properties found for device"))?;

            if properties.get::<bool>("is-default")? {
                let mut node_name = properties.get::<String>("node.name")?;

                if device_class == AudioDeviceClass::Sink {
                    node_name.push_str(".monitor");
                }

                return Ok(node_name);
            }
        }
    }

    Err(anyhow::anyhow!(
        "Failed to found audio device for class `{preferred_device_class:?}`"
    ))
}

fn gst_element_factory_make(factory_name: &str) -> anyhow::Result<gst::Element> {
    gst::ElementFactory::make(factory_name, None)
        .with_context(|| format!("Failed to make `{}`", factory_name))
}

async fn create_pipeline(
    stream: &gio::MemoryOutputStream,
    preferred_device_class: AudioDeviceClass,
) -> anyhow::Result<gst::Pipeline> {
    let pipeline = gst::Pipeline::new(None);

    let pulsesrc = gst_element_factory_make("pulsesrc")?;
    let audioconvert = gst_element_factory_make("audioconvert")?;
    let level = gst_element_factory_make("level")?;
    let opusenc = gst_element_factory_make("opusenc")?;
    let oggmux = gst_element_factory_make("oggmux")?;
    let giostreamsink = gst_element_factory_make("giostreamsink")?;

    match THREAD_POOL
        .push_future(move || find_default_device_name(preferred_device_class))?
        .await
    {
        Ok(ref device_name) => {
            log::info!("Using device `{device_name}` for recording");
            pulsesrc.set_property("device", device_name);
        }
        Err(err) => {
            // TODO Show userfacing error
            log::warn!("Failed to get default device name: {err:?}");
        }
    }

    level.set_property("interval", Duration::from_millis(80).as_nanos() as u64);
    level.set_property("peak-ttl", Duration::from_millis(80).as_nanos() as u64);
    opusenc.set_property("bitrate", 16_000);
    giostreamsink.set_property("stream", stream);

    let elements = [
        &pulsesrc,
        &audioconvert,
        &level,
        &opusenc,
        &oggmux,
        &giostreamsink,
    ];
    pipeline.add_many(&elements)?;

    pulsesrc.link_filtered(
        &audioconvert,
        &gst::Caps::builder("audio/x-raw")
            .field("channels", 1)
            .field("rate", 16_000)
            .build(),
    )?;
    audioconvert.link(&level)?;
    level.link(&opusenc)?;
    opusenc.link_filtered(&oggmux, &gst::Caps::builder("audio/x-opus").build())?;
    oggmux.link_filtered(&giostreamsink, &gst::Caps::builder("audio/ogg").build())?;

    for e in elements {
        e.sync_state_with_parent()?;
    }

    Ok(pipeline)
}
