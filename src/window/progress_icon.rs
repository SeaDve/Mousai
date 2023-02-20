// Based on code from Authenticator GPLv3
// See https://gitlab.gnome.org/World/Authenticator/-/blob/e5c9b9216094b33cceee59e095485cfbe5737252/src/widgets/progress_icon.rs

use gtk::{gdk, glib, graphene, gsk, prelude::*, subclass::prelude::*};

use std::{cell::Cell, cmp};

mod imp {
    use super::*;

    #[derive(Debug, Default, glib::Properties)]
    #[properties(wrapper_type = super::ProgressIcon)]
    pub struct ProgressIcon {
        #[property(get, set = Self::set_progress, minimum = 0.0, maximum = 1.0, explicit_notify)]
        pub(super) progress: Cell<f32>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for ProgressIcon {
        const NAME: &'static str = "MsaiProgressIcon";
        type Type = super::ProgressIcon;
        type ParentType = gtk::Widget;
    }

    impl ObjectImpl for ProgressIcon {
        crate::derived_properties!();
    }

    impl WidgetImpl for ProgressIcon {
        fn snapshot(&self, snapshot: &gtk::Snapshot) {
            let obj = self.obj();

            let progress = 1.0 - obj.progress();
            let color = obj.style_context().color();

            let color_stop = gsk::ColorStop::new(
                progress,
                gdk::RGBA::new(color.red(), color.green(), color.blue(), 0.15),
            );
            let color_stop_end = gsk::ColorStop::new(
                progress,
                gdk::RGBA::new(color.red(), color.green(), color.blue(), 1.0),
            );

            let size = obj.size() as f32;
            let radius = size / 2.0;
            let bounds = graphene::Rect::new(0.0, 0.0, size, size);
            let center = graphene::Point::new(radius, radius);
            snapshot.push_rounded_clip(&gsk::RoundedRect::from_rect(bounds, radius));
            snapshot.append_conic_gradient(&bounds, &center, 0.0, &[color_stop, color_stop_end]);
            snapshot.pop();
        }

        fn measure(&self, _orientation: gtk::Orientation, _for_size: i32) -> (i32, i32, i32, i32) {
            let size = self.obj().size();
            (size, size, -1, -1)
        }
    }

    impl ProgressIcon {
        fn set_progress(&self, progress: f32) {
            if (progress - self.progress.get()).abs() < f32::EPSILON {
                return;
            }

            let obj = self.obj();
            self.progress.replace(progress.clamp(0.0, 1.0));
            obj.queue_draw();
            obj.notify_progress();
        }
    }
}

glib::wrapper! {
     pub struct ProgressIcon(ObjectSubclass<imp::ProgressIcon>)
        @extends gtk::Widget;
}

impl ProgressIcon {
    pub fn new() -> Self {
        glib::Object::new()
    }

    fn size(&self) -> i32 {
        let width = self.width_request();
        let height = self.height_request();

        cmp::max(16, cmp::min(width, height))
    }
}

impl Default for ProgressIcon {
    fn default() -> Self {
        Self::new()
    }
}
