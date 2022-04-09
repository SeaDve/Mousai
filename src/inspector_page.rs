use adw::prelude::*;
use gtk::{glib, subclass::prelude::*};

use std::cell::RefCell;

use crate::recognizer::{ProviderType, PROVIDER_MANAGER};

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
                    glib::ParamSpecString::new(
                        "title",
                        "Title",
                        "Title of this inspector page",
                        Some(INSPECTOR_TITLE),
                        glib::ParamFlags::READABLE,
                    ),
                    // gtk-inspector-page uses this property
                    // So add it to avoid warnings
                    glib::ParamSpecObject::new(
                        "object",
                        "Object",
                        "Object",
                        glib::Object::static_type(),
                        glib::ParamFlags::READWRITE,
                    ),
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
        }

        fn dispose(&self, obj: &Self::Type) {
            while let Some(child) = obj.first_child() {
                child.unparent();
            }

            PROVIDER_MANAGER.reset_active();
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

    fn setup_provider_row(&self) {
        let imp = self.imp();

        imp.provider_row
            .set_model(Some(&adw::EnumListModel::new(ProviderType::static_type())));

        imp.provider_row
            .set_selected(PROVIDER_MANAGER.active() as u32);

        imp.provider_row
            .set_expression(Some(&gtk::ClosureExpression::new::<
                glib::GString,
                _,
                gtk::Expression,
            >(
                [],
                glib::closure!(|list_item: adw::EnumListItem| { list_item.name() }),
            )));

        imp.provider_row.connect_selected_notify(|provider_row| {
            if let Some(ref item) = provider_row
                .selected_item()
                .and_then(|item| item.downcast::<adw::EnumListItem>().ok())
            {
                PROVIDER_MANAGER.set_active(item.value().into());
            } else {
                log::warn!("provider_row doesn't have a valid selected item");
                PROVIDER_MANAGER.reset_active();
            }
        });
    }
}

impl Default for InspectorPage {
    fn default() -> Self {
        Self::new()
    }
}
