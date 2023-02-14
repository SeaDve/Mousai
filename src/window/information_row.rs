use adw::{prelude::*, subclass::prelude::*};
use gtk::glib;

use std::cell::RefCell;

mod imp {
    use super::*;

    #[derive(Debug, Default, glib::Properties, gtk::CompositeTemplate)]
    #[properties(wrapper_type = super::InformationRow)]
    #[template(resource = "/io/github/seadve/Mousai/ui/information-row.ui")]
    pub struct InformationRow {
        /// Value of the information
        ///
        /// If this is empty, self will be hidden.
        #[property(get, set = Self::set_value, explicit_notify)]
        pub(super) value: RefCell<String>,

        #[template_child]
        pub(super) value_label: TemplateChild<gtk::Label>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for InformationRow {
        const NAME: &'static str = "MsaiInformationRow";
        type Type = super::InformationRow;
        type ParentType = adw::ActionRow;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for InformationRow {
        crate::derived_properties!();

        fn constructed(&self) {
            self.parent_constructed();

            self.obj().update_ui();
        }

        fn dispose(&self) {
            self.dispose_template();
        }
    }

    impl WidgetImpl for InformationRow {}
    impl ListBoxRowImpl for InformationRow {}
    impl PreferencesRowImpl for InformationRow {}
    impl ActionRowImpl for InformationRow {}

    impl InformationRow {
        fn set_value(&self, value: String) {
            let obj = self.obj();

            if value == obj.value() {
                return;
            }

            self.value.replace(value);
            obj.update_ui();
            obj.notify_value();
        }
    }
}

glib::wrapper! {
    pub struct InformationRow(ObjectSubclass<imp::InformationRow>)
        @extends gtk::Widget, adw::PreferencesRow;
}

impl InformationRow {
    pub fn new() -> Self {
        glib::Object::new()
    }

    fn update_ui(&self) {
        let value = self.value();
        self.set_visible(!value.trim().is_empty());
        self.imp().value_label.set_text(&value);
    }
}

impl Default for InformationRow {
    fn default() -> Self {
        Self::new()
    }
}
