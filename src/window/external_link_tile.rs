use gtk::{glib, prelude::*, subclass::prelude::*};
use once_cell::unsync::OnceCell;

use crate::model::ExternalLinkWrapper;

mod imp {
    use super::*;

    #[derive(Debug, Default, glib::Properties, gtk::CompositeTemplate)]
    #[properties(wrapper_type = super::ExternalLinkTile)]
    #[template(resource = "/io/github/seadve/Mousai/ui/external-link-tile.ui")]
    pub struct ExternalLinkTile {
        /// Link shown by Self
        #[property(get, set, construct_only)]
        pub(super) external_link: OnceCell<ExternalLinkWrapper>,

        #[template_child]
        pub(super) image: TemplateChild<gtk::Image>,
        #[template_child]
        pub(super) label: TemplateChild<gtk::Label>,
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
        crate::derived_properties!();

        fn constructed(&self) {
            self.parent_constructed();

            let obj = self.obj();

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
        glib::Object::builder()
            .property("external-link", external_link)
            .build()
    }
}
