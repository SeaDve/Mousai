// Based on code from GNOME Sound Recorder GPLv3
// Modified to be bidirectional
// See https://gitlab.gnome.org/GNOME/gnome-sound-recorder/-/blob/master/src/waveform.js

use gtk::{cairo, glib, graphene, prelude::*, subclass::prelude::*};

use std::{cell::RefCell, collections::VecDeque};

const GUTTER: f64 = 9.0;
const LINE_WIDTH: f64 = 3.0;

mod imp {
    use super::*;

    #[derive(Debug, Default)]
    pub struct AudioVisualizer {
        pub peaks: RefCell<VecDeque<f64>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for AudioVisualizer {
        const NAME: &'static str = "MsaiAudioVisualizer";
        type Type = super::AudioVisualizer;
        type ParentType = gtk::Widget;

        fn class_init(klass: &mut Self::Class) {
            klass.set_css_name("audiovisualizer");
        }
    }

    impl ObjectImpl for AudioVisualizer {}

    impl WidgetImpl for AudioVisualizer {
        fn snapshot(&self, obj: &Self::Type, snapshot: &gtk::Snapshot) {
            obj.on_snapshot(snapshot);
        }
    }
}

glib::wrapper! {
    pub struct AudioVisualizer(ObjectSubclass<imp::AudioVisualizer>)
        @extends gtk::Widget;
}

impl AudioVisualizer {
    pub fn new() -> Self {
        glib::Object::new(&[]).expect("Failed to create AudioVisualizer")
    }

    pub fn push_peak(&self, peak: f64) {
        let mut peaks = self.peaks_mut();

        if peaks.len() as i32 > self.allocated_width() / (2 * GUTTER as i32) {
            peaks.pop_front();
        }

        peaks.push_back(peak);

        self.queue_draw();
    }

    pub fn clear_peaks(&self) {
        self.peaks_mut().clear();

        self.queue_draw();
    }

    fn peaks(&self) -> std::cell::Ref<VecDeque<f64>> {
        self.imp().peaks.borrow()
    }

    fn peaks_mut(&self) -> std::cell::RefMut<VecDeque<f64>> {
        self.imp().peaks.borrow_mut()
    }

    fn on_snapshot(&self, snapshot: &gtk::Snapshot) {
        let width = self.width();
        let height = self.height();
        let color = self.style_context().color();

        let bounds = graphene::Rect::new(0.0, 0.0, width as f32, height as f32);
        let ctx = snapshot.append_cairo(&bounds);
        ctx.set_line_cap(cairo::LineCap::Round);
        ctx.set_line_width(LINE_WIDTH);

        let max_height = height as f64;
        let v_center = max_height / 2.0;
        let h_center = width as f64 / 2.0;

        let peaks = self.peaks();
        let peaks_len = peaks.len();

        let mut pointer_a = h_center;
        let mut pointer_b = h_center;

        for (index, peak) in peaks.iter().rev().enumerate() {
            // Add feathering on both sides
            let alpha = 1.0 - (index as f64 / peaks_len as f64);
            ctx.set_source_rgba(
                color.red() as f64,
                color.green() as f64,
                color.blue() as f64,
                alpha,
            );

            // Creates a logarithmic decrease
            // Starts at index 2 because log0 is undefined and log1 is 0
            let this_max_height = max_height.log(index as f64 + 2.0) * 18.0;

            ctx.move_to(pointer_a, v_center + peak * this_max_height);
            ctx.line_to(pointer_a, v_center - peak * this_max_height);
            ctx.stroke().unwrap();

            ctx.move_to(pointer_b, v_center + peak * this_max_height);
            ctx.line_to(pointer_b, v_center - peak * this_max_height);
            ctx.stroke().unwrap();

            pointer_a += GUTTER;
            pointer_b -= GUTTER;
        }
    }
}
