use adw::subclass::prelude::*;
use gtk::{
    gio,
    glib::{self, clone},
    prelude::*,
};

use crate::{
    about_window,
    config::{APP_ID, PKGDATADIR, PROFILE, VERSION},
    core::AlbumArtStore,
    inspector_page::InspectorPage,
    settings::Settings,
    window::Window,
};

mod imp {
    use super::*;
    use glib::WeakRef;
    use once_cell::unsync::OnceCell;

    #[derive(Debug, Default)]
    pub struct Application {
        pub window: OnceCell<WeakRef<Window>>,
        pub session: OnceCell<soup::Session>,
        pub album_art_store: OnceCell<AlbumArtStore>,
        pub settings: Settings,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Application {
        const NAME: &'static str = "MsaiApplication";
        type Type = super::Application;
        type ParentType = adw::Application;
    }

    impl ObjectImpl for Application {}

    impl ApplicationImpl for Application {
        fn activate(&self, obj: &Self::Type) {
            self.parent_activate(obj);

            if let Some(window) = obj.main_window() {
                window.present();
            }
        }

        fn startup(&self, obj: &Self::Type) {
            self.parent_startup(obj);

            gtk::Window::set_default_icon_name(APP_ID);

            obj.setup_gactions();
            obj.setup_accels();
            obj.setup_inspector_page();
        }
    }

    impl GtkApplicationImpl for Application {}
    impl AdwApplicationImpl for Application {}
}

glib::wrapper! {
    pub struct Application(ObjectSubclass<imp::Application>)
        @extends gio::Application, gtk::Application, adw::Application,
        @implements gio::ActionMap, gio::ActionGroup;
}

impl Application {
    pub fn new() -> Self {
        glib::Object::new(&[
            ("application-id", &Some(APP_ID)),
            ("flags", &gio::ApplicationFlags::empty()),
            ("resource-base-path", &Some("/io/github/seadve/Mousai/")),
        ])
        .expect("Application initialization failed.")
    }

    pub fn settings(&self) -> Settings {
        self.imp().settings.clone()
    }

    pub fn session(&self) -> &soup::Session {
        self.imp().session.get_or_init(soup::Session::new)
    }

    pub fn album_art_store(&self) -> anyhow::Result<&AlbumArtStore> {
        self.imp()
            .album_art_store
            .get_or_try_init(|| AlbumArtStore::new(self.session()))
    }

    pub fn show_error(&self, message: &str) {
        if let Some(window) = self.main_window() {
            window.show_error(message);
        } else {
            log::warn!("Failed to show error: MainWindow doesn't exist");
        }
    }

    pub fn add_toast(&self, toast: &adw::Toast) {
        if let Some(window) = self.main_window() {
            window.add_toast(toast);
        } else {
            log::warn!("Failed to add toast: MainWindow doesn't exist");
        }
    }

    pub fn run(&self) {
        log::info!("Mousai ({})", APP_ID);
        log::info!("Version: {} ({})", VERSION, PROFILE);
        log::info!("Datadir: {}", PKGDATADIR);

        ApplicationExtManual::run(self);
    }

    fn main_window(&self) -> Option<Window> {
        let main_window = self
            .imp()
            .window
            .get_or_init(|| Window::new(self).downgrade())
            .upgrade();

        if main_window.is_none() {
            log::warn!("Failed to upgrade WeakRef<Window>");
        }

        main_window
    }

    fn setup_gactions(&self) {
        let action_quit = gio::SimpleAction::new("quit", None);
        action_quit.connect_activate(clone!(@weak self as obj => move |_, _| {
            if let Some(ref main_window) = obj.main_window() {
                main_window.close();
            }
            obj.quit();
        }));
        self.add_action(&action_quit);

        let action_about = gio::SimpleAction::new("about", None);
        action_about.connect_activate(clone!(@weak self as obj => move |_, _| {
            about_window::present(obj.main_window().as_ref());
        }));
        self.add_action(&action_about);
    }

    fn setup_accels(&self) {
        self.set_accels_for_action("app.quit", &["<Control>q"]);
        self.set_accels_for_action("win.navigate-back", &["<Alt>Left", "Escape"]);
        self.set_accels_for_action("win.toggle-playback", &["<Control>space"]);
        self.set_accels_for_action("win.toggle-listen", &["<Control>r"]);
        self.set_accels_for_action("win.toggle-search", &["<Control>f"]);
    }

    fn setup_inspector_page(&self) {
        if gio::IOExtensionPoint::lookup("gtk-inspector-page").is_some() {
            gio::IOExtensionPoint::implement(
                "gtk-inspector-page",
                InspectorPage::static_type(),
                APP_ID,
                10,
            );
        } else {
            log::warn!("Failed to setup Mousai's inspector page. IOExtensionPoint `gtk-inspector-page` is likely not found.");
        }
    }
}

impl Default for Application {
    fn default() -> Self {
        debug_assert!(
            gtk::is_initialized_main_thread(),
            "Application can only be accessed in the main thread"
        );

        gio::Application::default().unwrap().downcast().unwrap()
    }
}
