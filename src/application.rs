use adw::{prelude::*, subclass::prelude::*};
use anyhow::{Context, Result};
use gtk::{gio, glib};
use once_cell::unsync::OnceCell;

use crate::{
    about,
    config::{APP_ID, PKGDATADIR, PROFILE, VERSION},
    core::AlbumArtStore,
    database::{self, EnvExt, Migrations},
    database_error_window::DatabaseErrorWindow,
    inspector_page::InspectorPage,
    model::SongList,
    recognizer::{Recognizer, Recordings},
    settings::Settings,
    window::Window,
};

mod imp {
    use super::*;
    use glib::WeakRef;

    #[derive(Debug, Default)]
    pub struct Application {
        pub(super) window: OnceCell<WeakRef<Window>>,
        pub(super) session: OnceCell<soup::Session>,
        pub(super) album_art_store: OnceCell<AlbumArtStore>,
        pub(super) settings: Settings,

        pub(super) env: OnceCell<heed::Env>,
        pub(super) song_history: OnceCell<SongList>,
        pub(super) saved_recordings: OnceCell<Recordings>,
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

            if self.env.get().is_some()
                && self.song_history.get().is_some()
                && self.saved_recordings.get().is_some()
            {
                self.obj().window().present();
            } else {
                // TODO don't spawn a new window if one is already open
                // or find a better solution in handling these errors
                DatabaseErrorWindow::new(&self.obj()).present();
            }
        }

        fn startup(&self) {
            self.parent_startup();

            gtk::Window::set_default_icon_name(APP_ID);

            let obj = self.obj();
            obj.setup_gactions();
            obj.setup_accels();

            setup_inspector_page();

            if let Err(err) = obj.setup_env() {
                tracing::error!("Failed to setup db env: {:?}", err);
            }
        }

        fn shutdown(&self) {
            if let Some(env) = self.env.get() {
                if let Err(err) = env.force_sync() {
                    tracing::error!("Failed to sync db env on shutdown: {:?}", err);
                }
            }

            tracing::info!("Shutting down");

            self.parent_shutdown();
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

    pub fn window(&self) -> Window {
        self.imp()
            .window
            .get_or_init(|| {
                let recognizer = Recognizer::new(self.saved_recordings());
                Window::new(self, self.song_history(), &recognizer).downgrade()
            })
            .upgrade()
            .unwrap()
    }

    pub fn session(&self) -> &soup::Session {
        self.imp().session.get_or_init(soup::Session::new)
    }

    pub fn album_art_store(&self) -> Result<&AlbumArtStore> {
        self.imp()
            .album_art_store
            .get_or_try_init(|| AlbumArtStore::new(self.session()))
    }

    pub fn settings(&self) -> Settings {
        self.imp().settings.clone()
    }

    pub fn run(&self) -> glib::ExitCode {
        tracing::info!("Mousai ({})", APP_ID);
        tracing::info!("Version: {} ({})", VERSION, PROFILE);
        tracing::info!("Datadir: {}", PKGDATADIR);

        ApplicationExtManual::run(self)
    }

    fn song_history(&self) -> &SongList {
        self.imp()
            .song_history
            .get()
            .expect("song history should be initialized on env setup")
    }

    fn saved_recordings(&self) -> &Recordings {
        self.imp()
            .saved_recordings
            .get()
            .expect("saved recordings should be initialized on env setup")
    }

    fn setup_env(&self) -> Result<()> {
        {
            let env = database::new_env()?;

            env.with_write_txn(|wtxn| {
                let migrations = Migrations::new();
                migrations
                    .run(&env, wtxn)
                    .context("Failed to run migrations")
            })?;

            // We might open a db in migrations and open the same db with different
            // types later on, which is not allowed when done within the same env.
            // To workaround this, we close the env and open a new one.
            env.prepare_for_closing().wait();
        }

        let imp = self.imp();
        let env = database::new_env()?;
        imp.env.set(env.clone()).unwrap();

        let song_history =
            SongList::load_from_env(env.clone()).context("Failed to load song history")?;
        self.imp().song_history.set(song_history).unwrap();

        let recordings = Recordings::load_from_env(env)?;
        self.imp().saved_recordings.set(recordings).unwrap();

        Ok(())
    }

    fn setup_gactions(&self) {
        let quit_action = gio::ActionEntry::builder("quit")
            .activate(|obj: &Self, _, _| {
                if let Some(window) = obj.imp().window.get().and_then(|window| window.upgrade()) {
                    window.close();
                }
                obj.quit();
            })
            .build();
        let about_action = gio::ActionEntry::builder("about")
            .activate(|obj: &Self, _, _| {
                about::present_window(Some(&obj.window()));
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
