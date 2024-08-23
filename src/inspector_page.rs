use adw::prelude::*;
use gtk::{
    glib::{self, clone, closure, gformat},
    subclass::prelude::*,
};

use std::{cell::RefCell, time::Duration};

use crate::recognizer::{ProviderSettings, ProviderType, TestProviderMode};

const INSPECTOR_TITLE: &str = "Mousai";

mod imp {
    use super::*;
    use std::marker::PhantomData;

    #[derive(Default, glib::Properties, gtk::CompositeTemplate)]
    #[properties(wrapper_type = super::InspectorPage)]
    #[template(resource = "/io/github/seadve/Mousai/ui/inspector-page.ui")]
    pub struct InspectorPage {
        /// Title of this inspector page
        #[property(get = |_| INSPECTOR_TITLE.to_string())]
        pub(super) title: PhantomData<String>,
        /// Required property for gtk-inspector-page
        #[property(get, set)]
        pub(super) object: RefCell<Option<glib::Object>>,

        #[template_child]
        pub(super) page: TemplateChild<adw::PreferencesPage>, // Unused
        #[template_child]
        pub(super) provider_row: TemplateChild<adw::ComboRow>,
        #[template_child]
        pub(super) provider_model: TemplateChild<adw::EnumListModel>,
        #[template_child]
        pub(super) test_provider_mode_row: TemplateChild<adw::ComboRow>,
        #[template_child]
        pub(super) test_provider_mode_model: TemplateChild<adw::EnumListModel>,
        #[template_child]
        pub(super) test_listen_duration_row: TemplateChild<adw::SpinRow>,
        #[template_child]
        pub(super) test_recognize_duration_row: TemplateChild<adw::SpinRow>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for InspectorPage {
        const NAME: &'static str = "MsaiInspectorPage";
        type Type = super::InspectorPage;
        type ParentType = gtk::Widget;

        fn class_init(klass: &mut Self::Class) {
            ProviderType::static_type();
            TestProviderMode::static_type();

            klass.bind_template();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    #[glib::derived_properties]
    impl ObjectImpl for InspectorPage {
        fn constructed(&self) {
            self.parent_constructed();

            let obj = self.obj();

            obj.setup_rows();

            obj.update_test_rows_sensitivity();
        }

        fn dispose(&self) {
            self.dispose_template();

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
        glib::Object::new()
    }

    fn update_test_rows_sensitivity(&self) {
        let imp = self.imp();
        let is_test = imp.provider_row.selected_item().is_some_and(|obj| {
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

    fn setup_rows(&self) {
        let imp = self.imp();

        imp.provider_row.set_selected(
            imp.provider_model
                .find_position(ProviderSettings::lock().active as i32),
        );
        imp.provider_row
            .set_expression(Some(&gtk::ClosureExpression::new::<glib::GString>(
                &[] as &[gtk::Expression],
                closure!(|list_item: adw::EnumListItem| {
                    if ProviderType::try_from(list_item.value())
                        .unwrap()
                        .to_provider()
                        .is_test()
                    {
                        gformat!("{} (Test)", list_item.name())
                    } else {
                        list_item.name()
                    }
                }),
            )));
        imp.provider_row.connect_selected_notify(clone!(
            #[weak(rename_to = obj)]
            self,
            move |provider_row| {
                if let Some(ref item) = provider_row.selected_item() {
                    ProviderSettings::lock().active = item
                        .downcast_ref::<adw::EnumListItem>()
                        .unwrap()
                        .value()
                        .try_into()
                        .unwrap();
                } else {
                    tracing::warn!("provider_row doesn't have a selected item");
                    ProviderSettings::lock().active = ProviderType::default();
                }
                obj.update_test_rows_sensitivity();
            }
        ));

        imp.test_provider_mode_row.set_selected(
            imp.test_provider_mode_model
                .find_position(ProviderSettings::lock().test_mode as i32),
        );
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
                    tracing::warn!("test_provider_row doesn't have a selected item");
                    ProviderSettings::lock().test_mode = TestProviderMode::default();
                }
            });

        imp.test_listen_duration_row
            .set_value(ProviderSettings::lock().test_listen_duration.as_secs() as f64);
        imp.test_listen_duration_row
            .connect_value_notify(|spin_button| {
                ProviderSettings::lock().test_listen_duration =
                    Duration::from_secs(spin_button.value() as u64);
            });

        imp.test_recognize_duration_row
            .set_value(ProviderSettings::lock().test_recognize_duration.as_secs() as f64);
        imp.test_recognize_duration_row
            .connect_value_notify(|spin_button| {
                ProviderSettings::lock().test_recognize_duration =
                    Duration::from_secs(spin_button.value() as u64);
            });
    }
}
