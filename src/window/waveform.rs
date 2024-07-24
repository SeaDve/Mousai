// Based on code from GNOME Sound Recorder GPLv3
// Modified to be bidirectional
// See https://gitlab.gnome.org/GNOME/gnome-sound-recorder/-/blob/5ffc0fc935b402483b82c42f7baec015af21cdd6/src/waveform.ts

use gtk::{glib, gsk, prelude::*, subclass::prelude::*};

use std::{cell::RefCell, collections::VecDeque};

const GUTTER: f32 = 10.0;
const LINE_WIDTH: f32 = 6.0;

const NATURAL_WIDTH: i32 = 300;
const NATURAL_HEIGHT: i32 = 240;

mod imp {
    use super::*;

    #[derive(Default)]
    pub struct Waveform {
        pub(super) peaks: RefCell<VecDeque<f32>>,
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
        fn measure(&self, orientation: gtk::Orientation, _for_size: i32) -> (i32, i32, i32, i32) {
            match orientation {
                gtk::Orientation::Horizontal => (-1, NATURAL_WIDTH, -1, -1),
                gtk::Orientation::Vertical => (-1, NATURAL_HEIGHT, -1, -1),
                _ => unreachable!(),
            }
        }

        fn snapshot(&self, snapshot: &gtk::Snapshot) {
            let obj = self.obj();
            let width = obj.width();
            let height = obj.height();
            let color = obj.color();

            let stroke = gsk::Stroke::builder(LINE_WIDTH)
                .line_cap(gsk::LineCap::Round)
                .build();

            let v_center = height as f32 / 2.0;
            let h_center = width as f32 / 2.0;

            let peaks = self.peaks.borrow();

            // Since we pop peaks only when the number of peaks exceeds on push, we need to
            // also handle the case where the window can be resized. This is done by clamping
            // here the number of peaks to the maximum number of peaks that can be drawn visibly.
            let n_peaks_to_draw = peaks.len().clamp(0, obj.max_n_peaks() as usize);

            // Use horizontal center as we start drawing lines from the center and work
            // towards the sides.
            let mut pointer = h_center;

            for (index, peak) in peaks.iter().take(n_peaks_to_draw).enumerate() {
                // Index is reversed so that the alpha value and height of the lines of the
                // first/older peaks are lower and shorter respectively.
                let rev_index = n_peaks_to_draw - index - 1;

                let line_height_factor =
                    adw::Easing::EaseInQuad.ease(rev_index as f64 / n_peaks_to_draw as f64) as f32;
                let line_height = (peak * v_center * line_height_factor).clamp(0.1, f32::MAX);

                let pb = gsk::PathBuilder::new();

                pb.move_to(pointer, v_center + line_height);
                pb.line_to(pointer, v_center - line_height);

                pb.move_to(width as f32 - pointer, v_center + line_height);
                pb.line_to(width as f32 - pointer, v_center - line_height);

                snapshot.append_stroke(
                    &pb.to_path(),
                    &stroke,
                    &color.with_alpha(color.alpha() * (rev_index as f32 / n_peaks_to_draw as f32)), // Add feathering
                );

                pointer += GUTTER;
            }
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

    pub fn push_peak(&self, peak: f32) {
        let mut peaks = self.imp().peaks.borrow_mut();

        if peaks.len() > self.max_n_peaks() as usize {
            peaks.pop_back();
        }

        peaks.push_front(peak);

        self.queue_draw();
    }

    pub fn clear_peaks(&self) {
        self.imp().peaks.borrow_mut().clear();

        self.queue_draw();
    }

    /// Returns the maximum number of peaks that can be drawn visibly.
    fn max_n_peaks(&self) -> u32 {
        self.width().unsigned_abs() / (2 * GUTTER as u32)
    }
}

impl Default for Waveform {
    fn default() -> Self {
        Self::new()
    }
}
