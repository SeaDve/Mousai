use gtk::{glib, prelude::*, subclass::prelude::*};

use std::cell::Cell;

use crate::core::ClockTime;

mod imp {
    use super::*;
    use gtk::CompositeTemplate;
    use once_cell::sync::Lazy;

    #[derive(Debug, Default, CompositeTemplate)]
    #[template(resource = "/io/github/seadve/Mousai/ui/time-label.ui")]
    pub struct TimeLabel {
        #[template_child]
        pub(super) label: TemplateChild<gtk::Label>,

        pub(super) time: Cell<ClockTime>,
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
        fn properties() -> &'static [glib::ParamSpec] {
            static PROPERTIES: Lazy<Vec<glib::ParamSpec>> = Lazy::new(|| {
                vec![
                    // Time shown by Self
                    glib::ParamSpecBoxed::builder::<ClockTime>("time")
                        .explicit_notify()
                        .build(),
                ]
            });

            PROPERTIES.as_ref()
        }

        fn set_property(&self, _id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
            let obj = self.obj();

            match pspec.name() {
                "time" => {
                    let time = value.get().unwrap();
                    obj.set_time(time);
                }
                _ => unimplemented!(),
            }
        }

        fn property(&self, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
            let obj = self.obj();

            match pspec.name() {
                "time" => obj.time().to_value(),
                _ => unimplemented!(),
            }
        }

        fn constructed(&self) {
            self.parent_constructed();

            self.obj().update_label();
        }

        fn dispose(&self) {
            self.label.unparent();
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
        glib::Object::builder().build()
    }

    pub fn set_time(&self, time: ClockTime) {
        if time == self.time() {
            return;
        }

        self.imp().time.set(time);
        self.update_label();
        self.notify("time");
    }

    pub fn time(&self) -> ClockTime {
        self.imp().time.get()
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
