use std::cell::RefCell;

use anyhow::{anyhow, ensure, Context, Result};
use gst::{bus::BusWatchGuard, prelude::*};
use gtk::{
    gio::{self, prelude::*},
    glib::{self, clone},
};

use crate::{
    device::{self, DeviceClass},
    settings::AudioSourceType,
};

#[derive(Default)]

pub struct Recorder {
    pipeline: RefCell<Option<(gst::Pipeline, BusWatchGuard, gio::MemoryOutputStream)>>,
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
        audio_source_type: AudioSourceType,
        peak_callback: impl Fn(f64) + 'static,
    ) -> Result<()> {
        ensure!(
            self.pipeline.borrow().is_none(),
            "there is already a recording in progress"
        );

        let output_stream = gio::MemoryOutputStream::new_resizable();
        let pipeline = create_pipeline(&output_stream, audio_source_type)?;

        let bus_watch_guard = pipeline
            .bus()
            .unwrap()
            .add_watch_local(clone!(
                #[weak]
                pipeline,
                #[upgrade_or_panic]
                move |_, message| handle_bus_message(&pipeline, message, &peak_callback)
            ))
            .unwrap();
        self.pipeline
            .replace(Some((pipeline.clone(), bus_watch_guard, output_stream)));

        pipeline.set_state(gst::State::Playing)?;

        Ok(())
    }

    pub fn stop(&self) -> Result<glib::Bytes> {
        let (pipeline, _bus_watch_guard, stream) = self
            .pipeline
            .take()
            .ok_or_else(|| anyhow!("Recording has not been started"))?;

        pipeline.set_state(gst::State::Null)?;
        stream.close(gio::Cancellable::NONE)?;

        Ok(stream.steal_as_bytes())
    }
}

fn handle_bus_message(
    pipeline: &gst::Pipeline,
    message: &gst::Message,
    peak_callback: &impl Fn(f64),
) -> glib::ControlFlow {
    use gst::MessageView;

    match message.view() {
        MessageView::Element(e) => {
            tracing::trace!("Received element message on bus: {:?}", e);

            if let Some(structure) = e.structure() {
                if structure.has_name("level") {
                    let peak = structure
                        .get::<&glib::ValueArray>("peak")
                        .unwrap()
                        .first()
                        .unwrap()
                        .get::<f64>()
                        .unwrap();
                    let normalized_peak = 10_f64.powf(peak / 20.0);
                    peak_callback(normalized_peak);
                }
            }

            glib::ControlFlow::Continue
        }
        MessageView::Eos(_) => {
            tracing::debug!("Eos signal received from record bus");

            glib::ControlFlow::Break
        }
        MessageView::Error(e) => {
            let current_state = pipeline.state(None);
            tracing::debug!(?current_state, debug = ?e.debug(), err = ?e.error(), "Received error at bus");

            // TODO handle these errors

            glib::ControlFlow::Break
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
                return glib::ControlFlow::Continue;
            }

            tracing::debug!(
                "Pipeline changed state from `{:?}` -> `{:?}`",
                sc.old(),
                sc.current(),
            );

            glib::ControlFlow::Continue
        }
        MessageView::Warning(w) => {
            tracing::warn!("Received warning message on bus: {:?}", w);
            glib::ControlFlow::Continue
        }
        MessageView::Info(i) => {
            tracing::debug!("Received info message on bus: {:?}", i);
            glib::ControlFlow::Continue
        }
        other => {
            tracing::trace!("Received other message on bus: {:?}", other);
            glib::ControlFlow::Continue
        }
    }
}

fn make_pulsesrc(audio_source_type: AudioSourceType) -> Result<gst::Element> {
    let pulsesrc = gst::ElementFactory::make("pulsesrc").build()?;

    match audio_source_type {
        AudioSourceType::DesktopAudio => {
            let device = device::find_default(DeviceClass::Sink)?;
            let pulsesink = device.create_element(None)?;

            let device_name = pulsesink
                .property::<Option<String>>("device")
                .context("No device name")?;
            ensure!(!device_name.is_empty(), "Empty device name");

            let monitor_name = format!("{}.monitor", device_name);
            pulsesrc.set_property("device", &monitor_name);

            tracing::debug!("Found desktop audio with name `{}`", monitor_name);
        }
        AudioSourceType::Microphone => {
            let device = device::find_default(DeviceClass::Source)?;
            device.reconfigure_element(&pulsesrc)?;

            let device_name = pulsesrc
                .property::<Option<String>>("device")
                .context("No device name")?;
            ensure!(!device_name.is_empty(), "Empty device name");

            tracing::debug!("Found microphone with name `{}`", device_name);
        }
    }

    Ok(pulsesrc)
}

fn create_pipeline(
    stream: &gio::MemoryOutputStream,
    audio_source_type: AudioSourceType,
) -> Result<gst::Pipeline> {
    let pipeline = gst::Pipeline::new();

    let pulsesrc = make_pulsesrc(audio_source_type)?;
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

    let elements = [
        &pulsesrc,
        &audioconvert,
        &level,
        &opusenc,
        &oggmux,
        &giostreamsink,
    ];
    pipeline.add_many(elements)?;

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
