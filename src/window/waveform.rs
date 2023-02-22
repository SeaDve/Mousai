// Based on code from GNOME Sound Recorder GPLv3
// Modified to be bidirectional
// See https://gitlab.gnome.org/GNOME/gnome-sound-recorder/-/blob/master/src/waveform.js

use gtk::{cairo, glib, graphene, prelude::*, subclass::prelude::*};

use std::{cell::RefCell, collections::VecDeque};

const GUTTER: f64 = 10.0;
const LINE_WIDTH: f64 = 6.0;

mod imp {
    use super::*;

    #[derive(Debug, Default)]
    pub struct Waveform {
        pub(super) peaks: RefCell<VecDeque<f64>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Waveform {
        const NAME: &'static str = "MsaiWaveform";
        type Type = super::Waveform;
        type ParentType = gtk::Widget;

        fn class_init(klass: &mut Self::Class) {
            klass.set_css_name("waveform");
        }
    }

    impl ObjectImpl for Waveform {}

    impl WidgetImpl for Waveform {
        fn snapshot(&self, snapshot: &gtk::Snapshot) {
            self.obj().on_snapshot(snapshot);
        }
    }
}

glib::wrapper! {
    pub struct Waveform(ObjectSubclass<imp::Waveform>)
        @extends gtk::Widget;
}

impl Waveform {
    pub fn new() -> Self {
        glib::Object::new()
    }

    pub fn push_peak(&self, peak: f64) {
        let mut peaks = self.imp().peaks.borrow_mut();

        if peaks.len() as i32 > self.allocated_width() / (2 * GUTTER as i32) {
            peaks.pop_front();
        }

        peaks.push_back(peak);

        self.queue_draw();
    }

    pub fn clear_peaks(&self) {
        self.imp().peaks.borrow_mut().clear();

        self.queue_draw();
    }

    fn on_snapshot(&self, snapshot: &gtk::Snapshot) {
        let width = self.width();
        let height = self.height();
        let color = self.style_context().color();

        let ctx =
            snapshot.append_cairo(&graphene::Rect::new(0.0, 0.0, width as f32, height as f32));
        ctx.set_line_cap(cairo::LineCap::Round);
        ctx.set_line_width(LINE_WIDTH);

        let v_center = height as f64 / 2.0;
        let h_center = width as f64 / 2.0;

        let peaks = self.imp().peaks.borrow();
        let peaks_len = peaks.len();

        let mut pointer = h_center;

        for (index, peak) in peaks.iter().enumerate().rev() {
            ctx.set_source_rgba(
                color.red() as f64,
                color.green() as f64,
                color.blue() as f64,
                index as f64 / peaks_len as f64, // Adds feathering on both sides
            );

            let peak_height = ease_in_quad(index as f64 / peaks_len as f64) * peak * v_center;

            ctx.move_to(pointer, v_center + peak_height);
            ctx.line_to(pointer, v_center - peak_height);
            ctx.stroke().unwrap();

            ctx.move_to(2.0 * h_center - pointer, v_center + peak_height);
            ctx.line_to(2.0 * h_center - pointer, v_center - peak_height);
            ctx.stroke().unwrap();

            pointer += GUTTER;
        }
    }
}

fn ease_in_quad(x: f64) -> f64 {
    x * x
}

impl Default for Waveform {
    fn default() -> Self {
        Self::new()
    }
}
