use adw::{prelude::*, subclass::prelude::*};
use gtk::glib;

mod imp {
    use super::*;

    #[derive(Default, gtk::CompositeTemplate)]
    #[template(resource = "/io/github/seadve/Mousai/ui/database-error-window.ui")]
    pub struct DatabaseErrorWindow;

    #[glib::object_subclass]
    impl ObjectSubclass for DatabaseErrorWindow {
        const NAME: &'static str = "MsaiDatabaseErrorWindow";
        type Type = super::DatabaseErrorWindow;
        type ParentType = adw::ApplicationWindow;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for DatabaseErrorWindow {}
    impl WidgetImpl for DatabaseErrorWindow {}
    impl WindowImpl for DatabaseErrorWindow {}
    impl ApplicationWindowImpl for DatabaseErrorWindow {}
    impl AdwApplicationWindowImpl for DatabaseErrorWindow {}
}

glib::wrapper! {
    pub struct DatabaseErrorWindow(ObjectSubclass<imp::DatabaseErrorWindow>)
        @extends gtk::Widget, gtk::Window, gtk::ApplicationWindow, adw::ApplicationWindow;
}

impl DatabaseErrorWindow {
    pub fn new(application: &impl IsA<gtk::Application>) -> Self {
        glib::Object::builder()
            .property("application", application)
            .build()
    }
}
