use adw::prelude::*;
use gtk::{
    glib::{self, clone},
    subclass::prelude::*,
};

use std::{cell::RefCell, time::Duration};

use crate::recognizer::{ProviderManager, ProviderType, TestProviderMode};

const INSPECTOR_TITLE: &str = "Mousai";

mod imp {
    use super::*;
    use gtk::CompositeTemplate;
    use once_cell::sync::Lazy;

    #[derive(Debug, Default, CompositeTemplate)]
    #[template(resource = "/io/github/seadve/Mousai/ui/inspector-page.ui")]
    pub struct InspectorPage {
        #[template_child]
        pub provider_row: TemplateChild<adw::ComboRow>,
        #[template_child]
        pub test_provider_mode_row: TemplateChild<adw::ComboRow>,
        #[template_child]
        pub test_listen_duration_row: TemplateChild<adw::ActionRow>,
        #[template_child]
        pub test_listen_duration_button: TemplateChild<gtk::SpinButton>,
        #[template_child]
        pub test_recognize_duration_row: TemplateChild<adw::ActionRow>,
        #[template_child]
        pub test_recognize_duration_button: TemplateChild<gtk::SpinButton>,

        pub object: RefCell<Option<glib::Object>>,
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
                        .flags(glib::ParamFlags::READABLE)
                        .build(),
                    // Property needed as a gtk-inspector-page
                    glib::ParamSpecObject::builder("object", glib::Object::static_type()).build(),
                ]
            });

            PROPERTIES.as_ref()
        }

        fn set_property(
            &self,
            _obj: &Self::Type,
            _id: usize,
            value: &glib::Value,
            pspec: &glib::ParamSpec,
        ) {
            match pspec.name() {
                "object" => {
                    let object = value.get().unwrap();
                    self.object.replace(object);
                }
                _ => unimplemented!(),
            }
        }

        fn property(&self, _obj: &Self::Type, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
            match pspec.name() {
                "title" => INSPECTOR_TITLE.to_value(),
                "object" => self.object.borrow().to_value(),
                _ => unimplemented!(),
            }
        }

        fn constructed(&self, obj: &Self::Type) {
            self.parent_constructed(obj);

            obj.setup_provider_row();
            obj.setup_test_provider_row();
            obj.setup_duration_ui();

            obj.update_test_rows_sensitivity();
        }

        fn dispose(&self, obj: &Self::Type) {
            while let Some(child) = obj.first_child() {
                child.unparent();
            }

            ProviderManager::lock().reset();
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
        glib::Object::new(&[]).expect("Failed to create InspectorPage")
    }

    fn update_test_rows_sensitivity(&self) {
        let imp = self.imp();
        let is_test = imp
            .provider_row
            .selected_item()
            .and_then(|item| item.downcast::<adw::EnumListItem>().ok())
            .map_or(false, |item| {
                ProviderType::from(item.value()).to_provider().is_test()
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
            .set_selected(ProviderManager::lock().active as u32);

        imp.provider_row
            .set_expression(Some(&gtk::ClosureExpression::new::<
                glib::GString,
                _,
                gtk::Expression,
            >(
                [],
                glib::closure!(|list_item: adw::EnumListItem| { list_item.name() }),
            )));

        imp.provider_row
            .connect_selected_notify(clone!(@weak self as obj => move |provider_row| {
                if let Some(ref item) = provider_row
                    .selected_item()
                    .and_then(|item| item.downcast::<adw::EnumListItem>().ok())
                {
                    obj.update_test_rows_sensitivity();
                    ProviderManager::lock().active = item.value().into();
                } else {
                    log::warn!("provider_row doesn't have a valid selected item");
                    ProviderManager::lock().active = Default::default();
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
            .set_selected(ProviderManager::lock().test_mode as u32);

        imp.test_provider_mode_row
            .set_expression(Some(&gtk::ClosureExpression::new::<
                glib::GString,
                _,
                gtk::Expression,
            >(
                [],
                glib::closure!(|list_item: adw::EnumListItem| { list_item.name() }),
            )));

        imp.test_provider_mode_row
            .connect_selected_notify(|test_provider_row| {
                if let Some(ref item) = test_provider_row
                    .selected_item()
                    .and_then(|item| item.downcast::<adw::EnumListItem>().ok())
                {
                    ProviderManager::lock().test_mode = item.value().into();
                } else {
                    log::warn!("test_provider_row doesn't have a valid selected item");
                    ProviderManager::lock().test_mode = Default::default();
                }
            });
    }

    fn setup_duration_ui(&self) {
        let imp = self.imp();

        imp.test_listen_duration_button
            .set_value(ProviderManager::lock().test_listen_duration.as_secs() as f64);

        imp.test_listen_duration_button
            .connect_value_changed(|spin_button| {
                ProviderManager::lock().test_listen_duration =
                    Duration::from_secs(spin_button.value_as_int() as u64);
            });

        imp.test_recognize_duration_button
            .set_value(ProviderManager::lock().test_recognize_duration.as_secs() as f64);

        imp.test_recognize_duration_button
            .connect_value_changed(|spin_button| {
                ProviderManager::lock().test_recognize_duration =
                    Duration::from_secs(spin_button.value_as_int() as u64);
            });
    }
}

impl Default for InspectorPage {
    fn default() -> Self {
        Self::new()
    }
}
