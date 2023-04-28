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
    preferences_window::PreferencesWindow,
    recognizer::Recordings,
    settings::Settings,
    window::Window,
};

mod imp {
    use super::*;
    use glib::WeakRef;

    #[derive(Default)]
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

            if let Some(window) = self.window.get() {
                let window = window.upgrade().unwrap();
                window.present();
                return;
            }

            let obj = self.obj();

            if let (Some(song_history), Some(recordings)) =
                (self.song_history.get(), self.saved_recordings.get())
            {
                let window = Window::new(&obj);
                window.bind_models(song_history, recordings);
                self.window.set(window.downgrade()).unwrap();
                window.present();
            } else {
                // TODO don't spawn a new window if one is already open
                // or find a better solution in handling these errors
                let err_window = DatabaseErrorWindow::new(&obj);
                err_window.present();
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
            .get()
            .expect("window must be initialized on activate")
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

    pub fn settings(&self) -> &Settings {
        &self.imp().settings
    }

    pub fn run(&self) -> glib::ExitCode {
        tracing::info!("Mousai ({})", APP_ID);
        tracing::info!("Version: {} ({})", VERSION, PROFILE);
        tracing::info!("Datadir: {}", PKGDATADIR);

        ApplicationExtManual::run(self)
    }

    fn setup_env(&self) -> Result<()> {
        {
            let env = database::new_env()?;

            env.with_write_txn(|wtxn| {
                let mut migrations = Migrations::new();
                migrations.add("SongList: SerdeBincode<Uid> -> UidCodec", |env, wtxn| {
                    use heed::types::SerdeBincode;

                    use crate::{
                        database::SONG_LIST_DB_NAME,
                        model::{Song, Uid, UidCodec},
                    };

                    if let Some(db) = env.open_poly_database(wtxn, Some(SONG_LIST_DB_NAME))? {
                        let new_items = db
                            .iter::<SerdeBincode<Uid>, SerdeBincode<Song>>(wtxn)
                            .context("Failed to iter db")?
                            .collect::<Result<Vec<_>, _>>()
                            .context("Failed to collect items")?;

                        db.clear(wtxn)?;

                        for (uid, song) in new_items {
                            db.put::<UidCodec, SerdeBincode<Song>>(wtxn, &uid, &song)
                                .context("Failed to put item")?;
                        }
                    }

                    Ok(())
                });
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
        imp.song_history.set(song_history).unwrap();

        let recordings = Recordings::load_from_env(env)?;
        imp.saved_recordings.set(recordings).unwrap();

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
        let show_preferences_action = gio::ActionEntry::builder("show-preferences")
            .activate(|obj: &Self, _, _| {
                let window = PreferencesWindow::new(obj.settings());
                window.set_transient_for(Some(&obj.window()));
                window.present();
            })
            .build();
        let show_about_action = gio::ActionEntry::builder("show-about")
            .activate(|obj: &Self, _, _| {
                about::present_window(Some(&obj.window()));
            })
            .build();
        self.add_action_entries([quit_action, show_preferences_action, show_about_action]);
    }

    fn setup_accels(&self) {
        self.set_accels_for_action("app.quit", &["<Control>q"]);
        self.set_accels_for_action("app.show-preferences", &["<Control>comma"]);
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
