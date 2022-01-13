mod song_cell;

use adw::subclass::prelude::*;
use gettextrs::gettext;
use gtk::{
    gio,
    glib::{self, clone},
    prelude::*,
    subclass::prelude::*,
};
use once_cell::unsync::OnceCell;

use std::cell::Cell;

use self::song_cell::SongCell;
use crate::{config::PROFILE, model::SongList, recognizer::Recognizer, spawn, Application};

mod imp {
    use super::*;
    use gtk::CompositeTemplate;
    use once_cell::sync::Lazy;

    #[derive(Debug, Default, CompositeTemplate)]
    #[template(resource = "/io/github/seadve/Mousai/ui/window.ui")]
    pub struct Window {
        #[template_child]
        pub history_view: TemplateChild<gtk::GridView>,
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

        pub history: OnceCell<SongList>,
        pub recognizer: Recognizer,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Window {
        const NAME: &'static str = "MsaiWindow";
        type Type = super::Window;
        type ParentType = adw::ApplicationWindow;

        fn class_init(klass: &mut Self::Class) {
            SongCell::static_type();
            Self::bind_template(klass);

            klass.install_action("win.toggle-listen", None, |obj, _, _| {
                obj.on_toggle_listen();
            });
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

            obj.load_window_size();

            match obj.load_history() {
                Ok(history) => self.history.set(history).unwrap(),
                Err(err) => log::error!("Failed to load history: {:?}", err),
            }

            obj.setup_recognizer();
            obj.setup_history_view();
        }
    }

    impl WidgetImpl for Window {}
    impl WindowImpl for Window {
        fn close_request(&self, obj: &Self::Type) -> gtk::Inhibit {
            if let Err(err) = obj.save_window_size() {
                log::warn!("Failed to save window state, {:?}", &err);
            }

            if let Err(err) = obj.save_history() {
                log::error!("Failed to save history: {:?}", err);
            }

            self.parent_close_request(obj)
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

    fn history(&self) -> SongList {
        let imp = imp::Window::from_instance(self);
        imp.history.get().unwrap().clone()
    }

    fn on_toggle_listen(&self) {
        let imp = imp::Window::from_instance(self);

        if imp.recognizer.is_listening() {
            spawn!(clone!(@weak imp.recognizer as recognizer => async move {
                recognizer.cancel().await;
                log::info!("Cancelled recognizing");
            }));
        } else if let Err(err) = imp.recognizer.listen() {
            self.show_error(&gettext("Failed to start recording"), &err.to_string());
            log::error!("Failed to start recording: {:?} \n(dbg {:#?})", err, err);
        }
    }

    fn on_listen_done(&self, recognizer: &Recognizer) {
        spawn!(clone!(@weak recognizer, @weak self as obj => async move {
            log::info!("Listen done");

            match recognizer.listen_finish().await {
                Ok(song) => {
                    obj.history().append(song);
                }
                Err(err) => {
                    // TODO improve errors (more specific)
                    obj.show_error(&gettext("Something went wrong"), &err.to_string());
                    log::error!("Something went wrong: {:?} \n(dbg {:#?})", err, err);
                }
            }
        }));
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

        if self.history().is_empty() {
            imp.stack.set_visible_child(&imp.empty_page.get());
        } else {
            imp.stack.set_visible_child(&imp.main_page.get());
        }
    }

    fn show_error(&self, text: &str, secondary_text: &str) {
        let error_dialog = gtk::MessageDialogBuilder::new()
            .text(text)
            .secondary_text(secondary_text)
            .buttons(gtk::ButtonsType::Ok)
            .message_type(gtk::MessageType::Error)
            .modal(true)
            .transient_for(self)
            .build();

        error_dialog.connect_response(|error_dialog, _| error_dialog.destroy());
        error_dialog.present();
    }

    fn load_history(&self) -> anyhow::Result<SongList> {
        let settings = Application::default().settings();
        let json_str = settings.string("history");
        let history: SongList = serde_json::from_str(&json_str)?;
        Ok(history)
    }

    fn save_history(&self) -> anyhow::Result<()> {
        let json_str = serde_json::to_string(&self.history())?;
        let settings = Application::default().settings();
        settings.set_string("history", &json_str)?;
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

    fn save_window_size(&self) -> Result<(), glib::BoolError> {
        let settings = Application::default().settings();

        let (width, height) = self.default_size();

        settings.set_int("window-width", width)?;
        settings.set_int("window-height", height)?;

        settings.set_boolean("is-maximized", self.is_maximized())?;

        Ok(())
    }

    fn setup_history_view(&self) {
        let imp = imp::Window::from_instance(self);

        let model = self.history();

        model.connect_items_changed(clone!(@weak self as obj => move |_, _, _, _| {
            obj.update_stack();
        }));

        let selection_model = gtk::NoSelection::new(Some(&model));

        imp.history_view.set_model(Some(&selection_model));
    }

    fn setup_recognizer(&self) {
        let imp = imp::Window::from_instance(self);

        imp.recognizer
            .bind_property("is-listening", self, "is-listening")
            .flags(glib::BindingFlags::SYNC_CREATE)
            .build()
            .unwrap();

        imp.recognizer
            .connect_listen_done(clone!(@weak self as obj => move |recognizer| {
                obj.on_listen_done(recognizer);
            }));
    }
}
