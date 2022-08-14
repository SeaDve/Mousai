use adw::{prelude::*, subclass::prelude::*};
use gtk::glib;

use std::cell::RefCell;

mod imp {
    use super::*;
    use gtk::CompositeTemplate;
    use once_cell::sync::Lazy;

    #[derive(Debug, Default, CompositeTemplate)]
    #[template(resource = "/io/github/seadve/Mousai/ui/information-row.ui")]
    pub struct InformationRow {
        #[template_child]
        pub(super) data_label: TemplateChild<gtk::Label>,

        pub(super) data: RefCell<Option<String>>,
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
                    glib::ParamSpecString::builder("data")
                        .flags(glib::ParamFlags::READWRITE | glib::ParamFlags::EXPLICIT_NOTIFY)
                        .build(),
                ]
            });

            PROPERTIES.as_ref()
        }

        fn set_property(
            &self,
            obj: &Self::Type,
            _id: usize,
            value: &glib::Value,
            pspec: &glib::ParamSpec,
        ) {
            match pspec.name() {
                "data" => {
                    let data = value.get().unwrap();
                    obj.set_data(data);
                }
                _ => unimplemented!(),
            }
        }

        fn property(&self, obj: &Self::Type, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
            match pspec.name() {
                "data" => obj.data().to_value(),
                _ => unimplemented!(),
            }
        }

        fn constructed(&self, obj: &Self::Type) {
            self.parent_constructed(obj);

            obj.update_ui();
        }

        fn dispose(&self, obj: &Self::Type) {
            while let Some(child) = obj.first_child() {
                child.unparent();
            }
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
        glib::Object::new(&[]).expect("Failed to create InformationRow")
    }

    pub fn set_data(&self, data: Option<&str>) {
        if data == self.data().as_deref() {
            return;
        }

        self.imp().data.replace(data.map(|data| data.to_string()));
        self.update_ui();
        self.notify("data");
    }

    pub fn data(&self) -> Option<String> {
        self.imp().data.borrow().clone()
    }

    fn update_ui(&self) {
        let data = self.data().filter(|data| !data.is_empty());

        self.set_visible(data.is_some());
        self.imp().data_label.set_text(&data.unwrap_or_default());
    }
}

impl Default for InformationRow {
    fn default() -> Self {
        Self::new()
    }
}
