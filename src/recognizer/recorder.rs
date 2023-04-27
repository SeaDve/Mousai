use anyhow::{anyhow, ensure, Result};
use gst::prelude::*;
use gtk::{
    gio::{self, prelude::*},
    glib::{self, clone},
};

use std::cell::RefCell;

#[derive(Default)]

pub struct Recorder {
    current_data: RefCell<Option<(gst::Pipeline, gio::MemoryOutputStream)>>,
}

impl Drop for Recorder {
    fn drop(&mut self) {
        if let Err(err) = self.stop() {
            tracing::debug!("Failed to stop on dispose: {:?}", err);
        }
    }
}

impl Recorder {
    pub fn start(
        &self,
        device_name: Option<&str>,
        peak_callback: impl Fn(f64) + 'static,
    ) -> Result<()> {
        ensure!(
            self.current_data.borrow().is_none(),
            "there is already a recording in progress"
        );

        let output_stream = gio::MemoryOutputStream::new_resizable();
        let pipeline = create_pipeline(&output_stream, device_name)?;

        pipeline
            .bus()
            .unwrap()
            .add_watch_local(
                clone!(@weak pipeline => @default-return Continue(false), move |_, message| {
                    handle_bus_message(&pipeline, message, &peak_callback)
                }),
            )
            .unwrap();
        pipeline.set_state(gst::State::Playing)?;

        self.current_data.replace(Some((pipeline, output_stream)));
        Ok(())
    }

    pub fn stop(&self) -> Result<glib::Bytes> {
        let (pipeline, stream) = self
            .current_data
            .take()
            .ok_or_else(|| anyhow!("Recording has not been started"))?;

        pipeline.set_state(gst::State::Null)?;
        stream.close(gio::Cancellable::NONE)?;

        let _ = pipeline.bus().unwrap().remove_watch();

        Ok(stream.steal_as_bytes())
    }
}

fn handle_bus_message(
    pipeline: &gst::Pipeline,
    message: &gst::Message,
    handler: &impl Fn(f64),
) -> Continue {
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
                    handler(normalized_peak);
                }
            }

            Continue(true)
        }
        MessageView::Eos(_) => {
            tracing::debug!("Eos signal received from record bus");

            Continue(false)
        }
        MessageView::Error(e) => {
            let current_state = pipeline.state(None);
            tracing::debug!(?current_state, debug = ?e.debug(), err = ?e.error(), "Received error at bus");

            // TODO handle these errors

            Continue(false)
        }
        MessageView::StateChanged(sc) => {
            if message.src() != Some(pipeline.upcast_ref::<gst::Object>()) {
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

fn create_pipeline(
    stream: &gio::MemoryOutputStream,
    device_name: Option<&str>,
) -> Result<gst::Pipeline> {
    let pipeline = gst::Pipeline::new(None);

    let pulsesrc = gst::ElementFactory::make("pulsesrc").build()?;
    let audioconvert = gst::ElementFactory::make("audioconvert").build()?;
    let level = gst::ElementFactory::make("level")
        .property("interval", gst::ClockTime::from_mseconds(80))
        .property("peak-ttl", gst::ClockTime::from_mseconds(80))
        .build()?;
    let opusenc = gst::ElementFactory::make("opusenc")
        .property("bitrate", 16_000)
        .build()?;
    let oggmux = gst::ElementFactory::make("oggmux").build()?;
    let giostreamsink = gst::ElementFactory::make("giostreamsink")
        .property("stream", stream)
        .build()?;

    if let Some(device_name) = device_name {
        pulsesrc.set_property("device", device_name);
        tracing::debug!("Using device `{}` for recording", device_name);
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
