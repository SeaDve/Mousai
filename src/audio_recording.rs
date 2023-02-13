use anyhow::{anyhow, ensure, Result};
use gst::prelude::*;
use gtk::{
    gio::{self, prelude::*},
    glib::{self, clone, closure_local, subclass::prelude::*},
};
use once_cell::unsync::OnceCell;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use std::time::Duration;

use crate::core::DateTime;

fn serialize_once_cell<S>(cell: &OnceCell<impl Serialize>, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    cell.get().serialize(serializer)
}

fn deserialize_once_cell<'de, D, T>(deserializer: D) -> Result<OnceCell<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    Ok(OnceCell::with_value(T::deserialize(deserializer)?))
}

fn serialize_once_cell_gbytes<S>(
    cell: &OnceCell<glib::Bytes>,
    serializer: S,
) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    cell.get().map(|b| b.as_ref()).serialize(serializer)
}

fn deserialize_once_cell_gbytes<'de, D>(deserializer: D) -> Result<OnceCell<glib::Bytes>, D::Error>
where
    D: Deserializer<'de>,
{
    Ok(OnceCell::with_value(glib::Bytes::from_owned(
        Vec::<u8>::deserialize(deserializer)?,
    )))
}

mod imp {
    use super::*;
    use glib::subclass::Signal;
    use once_cell::sync::Lazy;

    #[derive(Debug, Default, Serialize, Deserialize)]
    pub struct AudioRecording {
        #[serde(skip)]
        pub(super) data: OnceCell<(gst::Pipeline, gio::MemoryOutputStream)>,
        #[serde(
            serialize_with = "serialize_once_cell",
            deserialize_with = "deserialize_once_cell"
        )]
        pub(super) recorded_time: OnceCell<DateTime>,
        #[serde(
            serialize_with = "serialize_once_cell_gbytes",
            deserialize_with = "deserialize_once_cell_gbytes"
        )]
        pub(super) bytes: OnceCell<glib::Bytes>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for AudioRecording {
        const NAME: &'static str = "MsaiAudioRecording";
        type Type = super::AudioRecording;
    }

    impl ObjectImpl for AudioRecording {
        fn signals() -> &'static [Signal] {
            static SIGNALS: Lazy<Vec<Signal>> = Lazy::new(|| {
                vec![Signal::builder("peak")
                    .param_types([f64::static_type()])
                    .build()]
            });

            SIGNALS.as_ref()
        }

        fn dispose(&self) {
            if let Err(err) = self.obj().stop() {
                tracing::warn!("Failed to stop recording on dispose: {:?}", err);
            }

            tracing::trace!("Recording instance disposed");
        }
    }
}

glib::wrapper! {
    pub struct AudioRecording(ObjectSubclass<imp::AudioRecording>);
}

impl AudioRecording {
    pub fn new() -> Self {
        glib::Object::builder().build()
    }

    pub fn recorded_time(&self) -> Option<&DateTime> {
        self.imp().recorded_time.get()
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

    pub fn start(&self, device_name: Option<&str>) -> Result<()> {
        let imp = self.imp();

        ensure!(imp.bytes.get().is_none(), "Recording already done");
        ensure!(imp.data.get().is_none(), "Already started recording");

        let output_stream = gio::MemoryOutputStream::new_resizable();
        let pipeline = create_pipeline(&output_stream, device_name)?;

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

        imp.recorded_time.set(DateTime::now()).unwrap();

        imp.data.set((pipeline, output_stream)).unwrap();
        Ok(())
    }

    pub fn stop(&self) -> Result<()> {
        ensure!(self.imp().bytes.get().is_none(), "Recording already done");

        let imp = self.imp();

        let (pipeline, stream) = imp
            .data
            .get()
            .ok_or_else(|| anyhow!("Recording has not been started"))?;

        pipeline.set_state(gst::State::Null)?;
        stream.close(gio::Cancellable::NONE)?;

        let _ = pipeline.bus().unwrap().remove_watch();

        let bytes = stream.steal_as_bytes();
        tracing::debug!(
            "Stopped recording with size {}",
            glib::format_size(bytes.len() as u64)
        );

        imp.bytes.set(bytes).unwrap();

        Ok(())
    }

    pub fn to_base_64(&self) -> Result<glib::GString> {
        let bytes = self
            .imp()
            .bytes
            .get()
            .ok_or_else(|| anyhow!("Recording has not been started or finished"))?;

        Ok(glib::base64_encode(bytes))
    }

    fn pipeline(&self) -> Option<&gst::Pipeline> {
        self.imp().data.get().map(|(pipeline, _)| pipeline)
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
                tracing::debug!("Eos signal received from record bus");

                Continue(false)
            }
            MessageView::Error(e) => {
                let current_state = self.pipeline().map(|p| p.state(None));
                tracing::debug!(?current_state, debug = ?e.debug(), err = ?e.error(), "Received error at bus");

                // TODO show user facing

                if let Err(err) = self.stop() {
                    tracing::warn!("Failed to stop recording on error: {:?}", err);
                }

                Continue(false)
            }
            MessageView::StateChanged(sc) => {
                if message.src()
                    != self
                        .pipeline()
                        .map(|pipeline| pipeline.upcast_ref::<gst::Object>())
                {
                    tracing::trace!(
                        "`{}` changed state from `{:?}` -> `{:?}`",
                        message
                            .src()
                            .map_or_else(|| "<unknown source>".into(), |e| e.name()),
                        sc.old(),
                        sc.current(),
                    );
                    return Continue(true);
                }

                tracing::debug!(
                    "Pipeline changed state from `{:?}` -> `{:?}`",
                    sc.old(),
                    sc.current(),
                );

                Continue(true)
            }
            MessageView::Warning(w) => {
                tracing::warn!("Received warning message on bus: {:?}", w);
                Continue(true)
            }
            MessageView::Info(i) => {
                tracing::debug!("Received info message on bus: {:?}", i);
                Continue(true)
            }
            other => {
                tracing::trace!("Received other message on bus: {:?}", other);
                Continue(true)
            }
        }
    }
}

impl Default for AudioRecording {
    fn default() -> Self {
        Self::new()
    }
}

impl Serialize for AudioRecording {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.imp().serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for AudioRecording {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let deserialized_inner = imp::AudioRecording::deserialize(deserializer)?;

        let this = Self::new();
        let inner = this.imp();

        if let Some(bytes) = deserialized_inner.bytes.into_inner() {
            inner.bytes.set(bytes).unwrap();
        }

        if let Some(recorded_time) = deserialized_inner.recorded_time.into_inner() {
            inner.recorded_time.set(recorded_time).unwrap();
        }

        Ok(this)
    }
}

fn create_pipeline(
    stream: &gio::MemoryOutputStream,
    device_name: Option<&str>,
) -> Result<gst::Pipeline> {
    let pipeline = gst::Pipeline::new(None);

    let pulsesrc = gst::ElementFactory::make("pulsesrc").build()?;
    let audioconvert = gst::ElementFactory::make("audioconvert").build()?;
    let level = gst::ElementFactory::make("level")
        .property("interval", Duration::from_millis(80).as_nanos() as u64)
        .property("peak-ttl", Duration::from_millis(80).as_nanos() as u64)
        .build()?;
    let opusenc = gst::ElementFactory::make("opusenc")
        .property("bitrate", 16_000)
        .build()?;
    let oggmux = gst::ElementFactory::make("oggmux").build()?;
    let giostreamsink = gst::ElementFactory::make("giostreamsink")
        .property("stream", stream)
        .build()?;

    if let Some(device_name) = device_name {
        pulsesrc.set_property("device", &device_name);
        tracing::debug!("Using device `{device_name}` for recording");
    } else {
        tracing::warn!("Recording without pulsesrc `device` property set");
    }

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
