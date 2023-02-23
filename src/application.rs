use adw::{prelude::*, subclass::prelude::*};
use anyhow::{Error, Result};
use gtk::{gio, glib};

use crate::{
    about,
    config::{APP_ID, PKGDATADIR, PROFILE, VERSION},
    core::AlbumArtStore,
    debug_assert_or_log, debug_unreachable_or_log,
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
        pub(super) window: OnceCell<WeakRef<Window>>,
        pub(super) session: OnceCell<soup::Session>,
        pub(super) album_art_store: OnceCell<AlbumArtStore>,
        pub(super) settings: Settings,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Application {
        const NAME: &'static str = "MsaiApplication";
        type Type = super::Application;
        type ParentType = adw::Application;
    }

    impl ObjectImpl for Application {}

    impl ApplicationImpl for Application {
        fn activate(&self) {
            self.parent_activate();

            if let Some(window) = self.obj().main_window() {
                window.present();
            }
        }

        fn startup(&self) {
            self.parent_startup();

            gtk::Window::set_default_icon_name(APP_ID);

            let obj = self.obj();

            obj.setup_gactions();
            obj.setup_accels();

            setup_inspector_page();
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
        glib::Object::builder()
            .property("application-id", APP_ID)
            .property("resource-base-path", "/io/github/seadve/Mousai/")
            .build()
    }

    pub fn settings(&self) -> Settings {
        self.imp().settings.clone()
    }

    pub fn session(&self) -> &soup::Session {
        self.imp().session.get_or_init(soup::Session::new)
    }

    pub fn album_art_store(&self) -> Result<&AlbumArtStore> {
        self.imp()
            .album_art_store
            .get_or_try_init(|| AlbumArtStore::new(self.session()))
    }

    pub fn add_toast_error(&self, err: &Error) {
        let toast = adw::Toast::builder()
            .title(glib::markup_escape_text(&err.to_string()))
            .priority(adw::ToastPriority::High)
            .build();
        self.add_toast(toast);
    }

    pub fn add_toast(&self, toast: adw::Toast) {
        if let Some(window) = self.main_window() {
            window.add_toast(toast);
        } else {
            debug_unreachable_or_log!("failed to add toast: MainWindow doesn't exist");
        }
    }

    pub fn run(&self) -> glib::ExitCode {
        tracing::info!("Mousai ({})", APP_ID);
        tracing::info!("Version: {} ({})", VERSION, PROFILE);
        tracing::info!("Datadir: {}", PKGDATADIR);

        ApplicationExtManual::run(self)
    }

    fn main_window(&self) -> Option<Window> {
        let main_window = self
            .imp()
            .window
            .get_or_init(|| Window::new(self).downgrade())
            .upgrade();

        debug_assert_or_log!(main_window.is_some(), "failed to upgrade WeakRef<Window>");

        main_window
    }

    fn setup_gactions(&self) {
        let quit_action = gio::ActionEntry::builder("quit")
            .activate(|obj: &Self, _, _| {
                if let Some(ref main_window) = obj.main_window() {
                    main_window.close();
                }
                obj.quit();
            })
            .build();
        let about_action = gio::ActionEntry::builder("about")
            .activate(|obj: &Self, _, _| {
                about::present_window(obj.main_window().as_ref());
            })
            .build();
        self.add_action_entries([quit_action, about_action]);
    }

    fn setup_accels(&self) {
        self.set_accels_for_action("app.quit", &["<Control>q"]);
        self.set_accels_for_action("window.close", &["<Control>w"]);
        self.set_accels_for_action("win.navigate-back", &["<Alt>Left", "Escape"]);
        self.set_accels_for_action("win.navigate-forward", &["<Alt>Right"]);
        self.set_accels_for_action("win.toggle-playback", &["<Control>space"]);
        self.set_accels_for_action("win.toggle-recognize", &["<Control>r"]);
        self.set_accels_for_action("win.toggle-search", &["<Control>f"]);
    }
}

impl Default for Application {
    fn default() -> Self {
        Self::new()
    }
}

fn setup_inspector_page() {
    if gio::IOExtensionPoint::lookup("gtk-inspector-page").is_some() {
        gio::IOExtensionPoint::implement(
            "gtk-inspector-page",
            InspectorPage::static_type(),
            APP_ID,
            10,
        );
    } else {
        tracing::warn!("Failed to setup Mousai's inspector page. IOExtensionPoint `gtk-inspector-page` is likely not found.");
    }
}
