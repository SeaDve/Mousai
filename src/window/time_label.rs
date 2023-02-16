use gtk::{glib, prelude::*, subclass::prelude::*};

use std::cell::Cell;

use crate::core::ClockTime;

mod imp {
    use super::*;

    #[derive(Debug, Default, glib::Properties, gtk::CompositeTemplate)]
    #[properties(wrapper_type = super::TimeLabel)]
    #[template(resource = "/io/github/seadve/Mousai/ui/time-label.ui")]
    pub struct TimeLabel {
        /// Time shown by Self
        #[property(get, set = Self::set_time, explicit_notify)]
        pub(super) time: Cell<ClockTime>,

        #[template_child]
        pub(super) label: TemplateChild<gtk::Label>,
    }

    impl TimeLabel {
        fn set_time(&self, time: ClockTime) {
            let obj = self.obj();

            if time == obj.time() {
                return;
            }

            self.time.set(time);
            obj.update_label();
            obj.notify_time();
        }
    }

    #[glib::object_subclass]
    impl ObjectSubclass for TimeLabel {
        const NAME: &'static str = "MsaiTimeLabel";
        type Type = super::TimeLabel;
        type ParentType = gtk::Widget;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();

            klass.set_accessible_role(gtk::AccessibleRole::Label);
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for TimeLabel {
        crate::derived_properties!();

        fn constructed(&self) {
            self.parent_constructed();

            self.obj().update_label();
        }

        fn dispose(&self) {
            self.dispose_template();
        }
    }

    impl WidgetImpl for TimeLabel {}
}

glib::wrapper! {
    pub struct TimeLabel(ObjectSubclass<imp::TimeLabel>)
        @extends gtk::Widget;
}

impl TimeLabel {
    pub fn new() -> Self {
        glib::Object::new()
    }

    pub fn reset(&self) {
        self.set_time(ClockTime::ZERO);
    }

    fn update_label(&self) {
        let seconds = self.time().as_secs();
        let seconds_display = seconds % 60;
        let minutes_display = seconds / 60;
        let formatted_time = format!("{}∶{:02}", minutes_display, seconds_display);
        self.imp().label.set_label(&formatted_time);
    }
}

impl Default for TimeLabel {
    fn default() -> Self {
        Self::new()
    }
}
