use adw::{prelude::*, subclass::prelude::*};
use gtk::glib;

use std::cell::RefCell;

mod imp {
    use super::*;
    use once_cell::sync::Lazy;

    #[derive(Debug, Default, gtk::CompositeTemplate)]
    #[template(resource = "/io/github/seadve/Mousai/ui/information-row.ui")]
    pub struct InformationRow {
        #[template_child]
        pub(super) value_label: TemplateChild<gtk::Label>,

        pub(super) value: RefCell<Option<String>>,
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
        fn properties() -> &'static [glib::ParamSpec] {
            static PROPERTIES: Lazy<Vec<glib::ParamSpec>> = Lazy::new(|| {
                vec![
                    // The value of the information. If this is None or
                    // empty, self will be hidden.
                    glib::ParamSpecString::builder("value")
                        .explicit_notify()
                        .build(),
                ]
            });

            PROPERTIES.as_ref()
        }

        fn set_property(&self, _id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
            let obj = self.obj();

            match pspec.name() {
                "value" => {
                    let value = value.get().unwrap();
                    obj.set_value(value);
                }
                _ => unimplemented!(),
            }
        }

        fn property(&self, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
            let obj = self.obj();

            match pspec.name() {
                "value" => obj.value().to_value(),
                _ => unimplemented!(),
            }
        }

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
}

glib::wrapper! {
    pub struct InformationRow(ObjectSubclass<imp::InformationRow>)
        @extends gtk::Widget, adw::PreferencesRow;
}

impl InformationRow {
    pub fn new() -> Self {
        glib::Object::new()
    }

    pub fn set_value(&self, value: Option<&str>) {
        if value == self.value().as_deref() {
            return;
        }

        self.imp()
            .value
            .replace(value.map(|value| value.to_string()));
        self.update_ui();
        self.notify("value");
    }

    pub fn value(&self) -> Option<String> {
        self.imp().value.borrow().clone()
    }

    fn update_ui(&self) {
        let value = self.value().filter(|value| !value.is_empty());

        self.set_visible(value.is_some());
        self.imp().value_label.set_text(&value.unwrap_or_default());
    }
}

impl Default for InformationRow {
    fn default() -> Self {
        Self::new()
    }
}
