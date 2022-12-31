use adw::prelude::*;
use gtk::{
    glib::{self, clone, closure},
    subclass::prelude::*,
};

use std::{cell::RefCell, time::Duration};

use crate::recognizer::{ProviderSettings, ProviderType, TestProviderMode};

const INSPECTOR_TITLE: &str = "Mousai";

mod imp {
    use super::*;
    use gtk::CompositeTemplate;
    use once_cell::sync::Lazy;

    #[derive(Debug, Default, CompositeTemplate)]
    #[template(resource = "/io/github/seadve/Mousai/ui/inspector-page.ui")]
    pub struct InspectorPage {
        #[template_child]
        pub(super) provider_row: TemplateChild<adw::ComboRow>,
        #[template_child]
        pub(super) test_provider_mode_row: TemplateChild<adw::ComboRow>,
        #[template_child]
        pub(super) test_listen_duration_row: TemplateChild<adw::ActionRow>,
        #[template_child]
        pub(super) test_listen_duration_button: TemplateChild<gtk::SpinButton>,
        #[template_child]
        pub(super) test_recognize_duration_row: TemplateChild<adw::ActionRow>,
        #[template_child]
        pub(super) test_recognize_duration_button: TemplateChild<gtk::SpinButton>,

        pub(super) object: RefCell<Option<glib::Object>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for InspectorPage {
        const NAME: &'static str = "MsaiInspectorPage";
        type Type = super::InspectorPage;
        type ParentType = gtk::Widget;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for InspectorPage {
        fn properties() -> &'static [glib::ParamSpec] {
            static PROPERTIES: Lazy<Vec<glib::ParamSpec>> = Lazy::new(|| {
                vec![
                    // Title of this inspector page
                    glib::ParamSpecString::builder("title")
                        .default_value(Some(INSPECTOR_TITLE))
                        .read_only()
                        .build(),
                    // Property needed as a gtk-inspector-page
                    glib::ParamSpecObject::builder::<glib::Object>("object").build(),
                ]
            });

            PROPERTIES.as_ref()
        }

        fn set_property(&self, _id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
            match pspec.name() {
                "object" => {
                    let object = value.get().unwrap();
                    self.object.replace(object);
                }
                _ => unimplemented!(),
            }
        }

        fn property(&self, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
            match pspec.name() {
                "title" => INSPECTOR_TITLE.to_value(),
                "object" => self.object.borrow().to_value(),
                _ => unimplemented!(),
            }
        }

        fn constructed(&self) {
            self.parent_constructed();

            let obj = self.obj();

            obj.setup_provider_row();
            obj.setup_test_provider_row();
            obj.setup_duration_ui();

            obj.update_test_rows_sensitivity();
        }

        fn dispose(&self) {
            while let Some(child) = self.obj().first_child() {
                child.unparent();
            }

            ProviderSettings::lock().reset();
        }
    }

    impl WidgetImpl for InspectorPage {}
}

glib::wrapper! {
    pub struct InspectorPage(ObjectSubclass<imp::InspectorPage>)
        @extends gtk::Widget;
}

impl InspectorPage {
    pub fn new() -> Self {
        glib::Object::builder().build()
    }

    fn update_test_rows_sensitivity(&self) {
        let imp = self.imp();
        let is_test = imp.provider_row.selected_item().map_or(false, |obj| {
            let item = obj.downcast_ref::<adw::EnumListItem>().unwrap();
            ProviderType::try_from(item.value())
                .unwrap()
                .to_provider()
                .is_test()
        });

        imp.test_provider_mode_row.set_sensitive(is_test);
        imp.test_listen_duration_row.set_sensitive(is_test);
        imp.test_recognize_duration_row.set_sensitive(is_test);
    }

    fn setup_provider_row(&self) {
        let imp = self.imp();

        imp.provider_row
            .set_model(Some(&adw::EnumListModel::new(ProviderType::static_type())));

        imp.provider_row
            .set_selected(ProviderSettings::lock().active as u32);

        imp.provider_row
            .set_expression(Some(&gtk::ClosureExpression::new::<glib::GString>(
                &[] as &[gtk::Expression],
                closure!(|list_item: adw::EnumListItem| list_item.name()),
            )));

        imp.provider_row
            .connect_selected_notify(clone!(@weak self as obj => move |provider_row| {
                if let Some(ref item) = provider_row.selected_item() {
                    obj.update_test_rows_sensitivity();
                    ProviderSettings::lock().active = item
                        .downcast_ref::<adw::EnumListItem>()
                        .unwrap()
                        .value()
                        .try_into()
                        .unwrap();
                } else {
                    tracing::warn!("provider_row doesn't have a valid selected item");
                    ProviderSettings::lock().active = ProviderType::default();
                }
            }));
    }

    fn setup_test_provider_row(&self) {
        let imp = self.imp();

        imp.test_provider_mode_row
            .set_model(Some(&adw::EnumListModel::new(
                TestProviderMode::static_type(),
            )));

        imp.test_provider_mode_row
            .set_selected(ProviderSettings::lock().test_mode as u32);

        imp.test_provider_mode_row
            .set_expression(Some(&gtk::ClosureExpression::new::<glib::GString>(
                &[] as &[gtk::Expression],
                closure!(|list_item: adw::EnumListItem| list_item.name()),
            )));

        imp.test_provider_mode_row
            .connect_selected_notify(|test_provider_row| {
                if let Some(ref item) = test_provider_row.selected_item() {
                    ProviderSettings::lock().test_mode = item
                        .downcast_ref::<adw::EnumListItem>()
                        .unwrap()
                        .value()
                        .try_into()
                        .unwrap();
                } else {
                    tracing::warn!("test_provider_row doesn't have a valid selected item");
                    ProviderSettings::lock().test_mode = TestProviderMode::default();
                }
            });
    }

    fn setup_duration_ui(&self) {
        let imp = self.imp();

        imp.test_listen_duration_button
            .set_value(ProviderSettings::lock().test_listen_duration.as_secs() as f64);

        imp.test_listen_duration_button
            .connect_value_changed(|spin_button| {
                ProviderSettings::lock().test_listen_duration =
                    Duration::from_secs(spin_button.value_as_int() as u64);
            });

        imp.test_recognize_duration_button
            .set_value(ProviderSettings::lock().test_recognize_duration.as_secs() as f64);

        imp.test_recognize_duration_button
            .connect_value_changed(|spin_button| {
                ProviderSettings::lock().test_recognize_duration =
                    Duration::from_secs(spin_button.value_as_int() as u64);
            });
    }
}

impl Default for InspectorPage {
    fn default() -> Self {
        Self::new()
    }
}
