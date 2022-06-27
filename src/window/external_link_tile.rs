use gtk::{glib, prelude::*, subclass::prelude::*};
use once_cell::unsync::OnceCell;

use crate::model::ExternalLinkWrapper;

mod imp {
    use super::*;
    use glib::WeakRef;
    use gtk::CompositeTemplate;
    use once_cell::sync::Lazy;

    #[derive(Debug, Default, CompositeTemplate)]
    #[template(resource = "/io/github/seadve/Mousai/ui/external-link-tile.ui")]
    pub struct ExternalLinkTile {
        #[template_child]
        pub image: TemplateChild<gtk::Image>,
        #[template_child]
        pub label: TemplateChild<gtk::Label>,

        pub external_link: OnceCell<WeakRef<ExternalLinkWrapper>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for ExternalLinkTile {
        const NAME: &'static str = "MsaiExternalLinkTile";
        type Type = super::ExternalLinkTile;
        type ParentType = gtk::FlowBoxChild;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for ExternalLinkTile {
        fn properties() -> &'static [glib::ParamSpec] {
            static PROPERTIES: Lazy<Vec<glib::ParamSpec>> = Lazy::new(|| {
                vec![
                    // Link represented by Self
                    glib::ParamSpecObject::builder(
                        "external-link",
                        ExternalLinkWrapper::static_type(),
                    )
                    .flags(glib::ParamFlags::READWRITE | glib::ParamFlags::CONSTRUCT_ONLY)
                    .build(),
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
                "external-link" => {
                    let external_link: ExternalLinkWrapper = value.get().unwrap();
                    self.external_link.set(external_link.downgrade()).unwrap();
                }
                _ => unimplemented!(),
            }
        }

        fn property(&self, obj: &Self::Type, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
            match pspec.name() {
                "external-link" => obj.external_link().to_value(),
                _ => unimplemented!(),
            }
        }

        fn constructed(&self, obj: &Self::Type) {
            self.parent_constructed(obj);

            let external_link_wrapper = obj.external_link();
            let external_link = external_link_wrapper.inner();

            obj.add_css_class(external_link.css_class());
            obj.set_tooltip_text(Some(&external_link.tooltip_text()));
            self.image.set_icon_name(Some(external_link.icon_name()));
            self.label.set_label(&external_link.name());
        }
    }

    impl WidgetImpl for ExternalLinkTile {}
    impl FlowBoxChildImpl for ExternalLinkTile {}
}

glib::wrapper! {
    pub struct ExternalLinkTile(ObjectSubclass<imp::ExternalLinkTile>)
        @extends gtk::Widget, gtk::FlowBoxChild;
}

impl ExternalLinkTile {
    pub fn new(external_link: &ExternalLinkWrapper) -> Self {
        glib::Object::new(&[("external-link", external_link)])
            .expect("Failed to create ExternalLinkTile")
    }

    pub fn external_link(&self) -> ExternalLinkWrapper {
        self.imp()
            .external_link
            .get()
            .unwrap()
            .clone()
            .upgrade()
            .unwrap()
    }
}
