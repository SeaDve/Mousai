use adw::prelude::*;
use gtk::{
    gdk,
    glib::{self, clone},
    graphene,
    subclass::prelude::*,
};

use std::{
    cell::Cell,
    f64::consts::{FRAC_PI_2, TAU},
};

const SIZE: i32 = 16;

mod imp {
    use super::*;
    use glib::WeakRef;

    #[derive(Debug, Default, glib::Properties)]
    #[properties(wrapper_type = super::ProgressPaintable)]
    pub struct ProgressPaintable {
        #[property(get, set, construct_only)]
        pub(super) widget: WeakRef<gtk::Widget>,
        #[property(get, set = Self::set_progress, minimum = 0.0, maximum = 1.0, explicit_notify)]
        pub(super) progress: Cell<f64>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for ProgressPaintable {
        const NAME: &'static str = "MsaiProgressPaintable";
        type Type = super::ProgressPaintable;
        type Interfaces = (gdk::Paintable, gtk::SymbolicPaintable);
    }

    impl ObjectImpl for ProgressPaintable {
        crate::derived_properties!();

        fn constructed(&self) {
            let obj = self.obj();

            self.widget.upgrade().unwrap().connect_scale_factor_notify(
                clone!(@weak obj => move |_| {
                    obj.invalidate_size();
                }),
            );

            self.parent_constructed();
        }
    }

    impl PaintableImpl for ProgressPaintable {
        fn intrinsic_width(&self) -> i32 {
            SIZE * self.widget.upgrade().unwrap().scale_factor()
        }

        fn intrinsic_height(&self) -> i32 {
            SIZE * self.widget.upgrade().unwrap().scale_factor()
        }

        fn snapshot(&self, _snapshot: &gdk::Snapshot, _width: f64, _height: f64) {}
    }

    impl SymbolicPaintableImpl for ProgressPaintable {
        fn snapshot_symbolic(
            &self,
            snapshot: &gdk::Snapshot,
            width: f64,
            height: f64,
            colors: &[gdk::RGBA],
        ) {
            let cr =
                snapshot.append_cairo(&graphene::Rect::new(0.0, 0.0, width as f32, height as f32));

            let color = colors[0];
            let progress = self.progress.get();
            let arc_end = progress * TAU - FRAC_PI_2;

            let cx = width / 2.0;
            let cy = height / 2.0;
            let radius = width / 2.0;

            cr.set_source_rgba(
                color.red() as f64,
                color.green() as f64,
                color.blue() as f64,
                color.alpha() as f64,
            );
            cr.move_to(cx, cy);
            cr.arc(cx, cy, radius, -FRAC_PI_2, arc_end);
            cr.fill().unwrap();

            cr.set_source_rgba(
                color.red() as f64,
                color.green() as f64,
                color.blue() as f64,
                color.alpha() as f64 * 0.15,
            );
            cr.move_to(cx, cy);
            cr.arc(cx, cy, radius, arc_end, 3.0 * FRAC_PI_2);
            cr.fill().unwrap();
        }
    }

    impl ProgressPaintable {
        fn set_progress(&self, progress: f64) {
            if (progress - self.progress.get()).abs() < f64::EPSILON {
                return;
            }

            let obj = self.obj();
            self.progress.replace(progress);
            obj.invalidate_contents();
            obj.notify_progress();
        }
    }
}

glib::wrapper! {
     pub struct ProgressPaintable(ObjectSubclass<imp::ProgressPaintable>)
        @implements gdk::Paintable, gtk::SymbolicPaintable;
}

impl ProgressPaintable {
    pub fn new(widget: &impl IsA<gtk::Widget>) -> Self {
        glib::Object::builder().property("widget", widget).build()
    }
}
