mod history_view;

use adw::subclass::prelude::*;
use gettextrs::gettext;
use gtk::{
    gio,
    glib::{self, clone},
    prelude::*,
    subclass::prelude::*,
};

use std::cell::Cell;

use self::history_view::HistoryView;
use crate::{config::PROFILE, Application};

mod imp {
    use super::*;
    use gtk::CompositeTemplate;
    use once_cell::sync::Lazy;

    #[derive(Debug, Default, CompositeTemplate)]
    #[template(resource = "/io/github/seadve/Mousai/ui/window.ui")]
    pub struct Window {
        #[template_child]
        pub history_view: TemplateChild<HistoryView>,
        #[template_child]
        pub listen_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub stack: TemplateChild<gtk::Stack>,
        #[template_child]
        pub main_page: TemplateChild<gtk::ScrolledWindow>,
        #[template_child]
        pub empty_page: TemplateChild<adw::StatusPage>,
        #[template_child]
        pub listening_page: TemplateChild<adw::StatusPage>,

        pub is_listening: Cell<bool>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Window {
        const NAME: &'static str = "MsaiWindow";
        type Type = super::Window;
        type ParentType = adw::ApplicationWindow;

        fn class_init(klass: &mut Self::Class) {
            Self::bind_template(klass);

            klass.install_property_action("win.toggle-listen", "is-listening");
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for Window {
        fn properties() -> &'static [glib::ParamSpec] {
            static PROPERTIES: Lazy<Vec<glib::ParamSpec>> = Lazy::new(|| {
                vec![glib::ParamSpec::new_boolean(
                    "is-listening",
                    "Is Listening",
                    "Whether Self is in listening state",
                    false,
                    glib::ParamFlags::READWRITE | glib::ParamFlags::EXPLICIT_NOTIFY,
                )]
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
                "is-listening" => {
                    let is_listening = value.get().unwrap();
                    obj.set_is_listening(is_listening);
                }
                _ => unimplemented!(),
            }
        }

        fn property(&self, obj: &Self::Type, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
            match pspec.name() {
                "is-listening" => obj.is_listening().to_value(),
                _ => unimplemented!(),
            }
        }

        fn constructed(&self, obj: &Self::Type) {
            self.parent_constructed(obj);

            if PROFILE == "Devel" {
                obj.add_css_class("devel");
            }

            obj.setup_history_view();
            obj.load_window_size();

            obj.set_is_listening(false);
        }
    }

    impl WidgetImpl for Window {}
    impl WindowImpl for Window {
        fn close_request(&self, window: &Self::Type) -> gtk::Inhibit {
            if let Err(err) = window.save_window_size() {
                log::warn!("Failed to save window state, {}", &err);
            }

            self.parent_close_request(window)
        }
    }

    impl ApplicationWindowImpl for Window {}
    impl AdwApplicationWindowImpl for Window {}
}

glib::wrapper! {
    pub struct Window(ObjectSubclass<imp::Window>)
        @extends gtk::Widget, gtk::Window, gtk::ApplicationWindow, adw::ApplicationWindow,
        @implements gio::ActionMap, gio::ActionGroup;
}

impl Window {
    pub fn new(app: &Application) -> Self {
        glib::Object::new(&[("application", app)]).expect("Failed to create Window")
    }

    pub fn set_is_listening(&self, is_listening: bool) {
        let imp = imp::Window::from_instance(self);
        imp.is_listening.set(is_listening);
        self.notify("is-listening");

        log::debug!("is_listening: {}", is_listening);

        self.update_listen_button();
        self.update_stack();
    }

    pub fn is_listening(&self) -> bool {
        let imp = imp::Window::from_instance(self);
        imp.is_listening.get()
    }

    fn update_listen_button(&self) {
        let imp = imp::Window::from_instance(self);

        if self.is_listening() {
            imp.listen_button.remove_css_class("suggested-action");
            imp.listen_button.add_css_class("destructive-action");

            imp.listen_button.set_label(&gettext("Cancel"));
            imp.listen_button
                .set_tooltip_text(Some(&gettext("Start Identifying Music")));
        } else {
            imp.listen_button.remove_css_class("destructive-action");
            imp.listen_button.add_css_class("suggested-action");

            imp.listen_button.set_label(&gettext("Listen"));
            imp.listen_button
                .set_tooltip_text(Some(&gettext("Cancel Listening")));
        }
    }

    fn update_stack(&self) {
        let imp = imp::Window::from_instance(self);

        if self.is_listening() {
            imp.stack.set_visible_child(&imp.listening_page.get());
            return;
        }

        if let Some(ref model) = imp.history_view.model() {
            if model.is_empty() {
                imp.stack.set_visible_child(&imp.empty_page.get());
            } else {
                imp.stack.set_visible_child(&imp.main_page.get());
            }
        } else {
            imp.stack.set_visible_child(&imp.empty_page.get());
        }
    }

    fn setup_history_view(&self) {
        let imp = imp::Window::from_instance(self);

        let model = crate::model::SongList::new();

        model.append(crate::model::Song::new("A song", "Someone", ""));
        model.append(crate::model::Song::new("Another song", "Someone else", ""));

        model.connect_items_changed(clone!(@weak self as obj => move |_, _, _, _| {
            obj.update_stack();
        }));

        imp.history_view.set_model(Some(&model));
    }

    fn save_window_size(&self) -> Result<(), glib::BoolError> {
        let settings = Application::default().settings();

        let (width, height) = self.default_size();

        settings.set_int("window-width", width)?;
        settings.set_int("window-height", height)?;

        settings.set_boolean("is-maximized", self.is_maximized())?;

        Ok(())
    }

    fn load_window_size(&self) {
        let settings = Application::default().settings();

        let width = settings.int("window-width");
        let height = settings.int("window-height");
        let is_maximized = settings.boolean("is-maximized");

        self.set_default_size(width, height);

        if is_maximized {
            self.maximize();
        }
    }
}
