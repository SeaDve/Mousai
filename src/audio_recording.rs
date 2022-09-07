use anyhow::{anyhow, ensure, Context, Result};
use gst::prelude::*;
use gtk::{
    gio::{self, prelude::*},
    glib::{self, clone, closure_local, subclass::prelude::*},
};
use once_cell::unsync::OnceCell;

use std::{cell::Cell, time::Duration};

mod imp {
    use super::*;
    use glib::subclass::Signal;
    use once_cell::sync::Lazy;

    #[derive(Debug, Default)]
    pub struct AudioRecording {
        pub(super) data: OnceCell<(gst::Pipeline, gio::MemoryOutputStream)>,
        pub(super) is_done: Cell<bool>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for AudioRecording {
        const NAME: &'static str = "MsaiAudioRecording";
        type Type = super::AudioRecording;
    }

    impl ObjectImpl for AudioRecording {
        fn signals() -> &'static [Signal] {
            static SIGNALS: Lazy<Vec<Signal>> = Lazy::new(|| {
                vec![Signal::builder(
                    "peak",
                    &[f64::static_type().into()],
                    <()>::static_type().into(),
                )
                .build()]
            });

            SIGNALS.as_ref()
        }

        fn dispose(&self, obj: &Self::Type) {
            if let Err(err) = obj.stop() {
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
        glib::Object::new(&[]).expect("Failed to create AudioRecording.")
    }

    pub fn start(&self, device_name: Option<&str>) -> Result<()> {
        let imp = self.imp();

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

        imp.data.set((pipeline, output_stream)).unwrap();
        Ok(())
    }

    pub fn stop(&self) -> Result<()> {
        let imp = self.imp();

        let (pipeline, stream) = imp
            .data
            .get()
            .ok_or_else(|| anyhow!("Recording has not been started"))?;

        pipeline.set_state(gst::State::Null)?;
        stream.close(gio::Cancellable::NONE)?;

        let _ = pipeline.bus().unwrap().remove_watch();

        imp.is_done.set(true);
        tracing::debug!("Stopped recording");

        Ok(())
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

    pub fn to_base_64(&self) -> Result<glib::GString> {
        let imp = self.imp();

        let (_, stream) = imp
            .data
            .get()
            .ok_or_else(|| anyhow!("Recording has not been started"))?;

        ensure!(imp.is_done.get(), "Recording is not done");

        let bytes = stream.steal_as_bytes();
        tracing::debug!(
            "Recorded audio with size {}",
            glib::format_size(bytes.len() as u64)
        );

        Ok(glib::base64_encode(&bytes))
    }

    fn pipeline(&self) -> Option<&gst::Pipeline> {
        self.imp().data.get().map(|(pipeline, _)| pipeline)
    }

    fn handle_bus_message(&self, message: &gst::Message) -> Continue {
        use gst::MessageView;

        let imp = self.imp();

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
                if message.src().as_ref()
                    != imp
                        .data
                        .get()
                        .map(|(pipeline, _)| pipeline.upcast_ref::<gst::Object>())
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

fn element_factory_make(factory_name: &str) -> Result<gst::Element> {
    gst::ElementFactory::make(factory_name, None)
        .with_context(|| format!("Failed to make `{}`", factory_name))
}

fn create_pipeline(
    stream: &gio::MemoryOutputStream,
    device_name: Option<&str>,
) -> Result<gst::Pipeline> {
    let pipeline = gst::Pipeline::new(None);

    let pulsesrc = element_factory_make("pulsesrc")?;
    let audioconvert = element_factory_make("audioconvert")?;
    let level = element_factory_make("level")?;
    let opusenc = element_factory_make("opusenc")?;
    let oggmux = element_factory_make("oggmux")?;
    let giostreamsink = element_factory_make("giostreamsink")?;

    if let Some(device_name) = device_name {
        pulsesrc.set_property("device", &device_name);
        tracing::debug!("Using device `{device_name}` for recording");
    } else {
        tracing::warn!("Recording without pulsesrc `device` property set");
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
