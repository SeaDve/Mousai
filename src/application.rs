use adw::{prelude::*, subclass::prelude::*};
use anyhow::{Context, Result};
use gtk::{gio, glib};
use soup::prelude::*;

use std::{cell::OnceCell, time::Instant};

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
        pub(super) session: OnceCell<(soup::Session, soup::Cache)>,
        pub(super) album_art_store: OnceCell<AlbumArtStore>,
        pub(super) env: OnceCell<(heed::Env, SongList, Recordings)>,
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

            if let Some(window) = self.window.get() {
                debug_assert!(self.env.get().is_some(), "env must be initialized too");

                let window = window.upgrade().unwrap();
                window.present();
                return;
            }

            let obj = self.obj();

            // TODO use `get_or_try_init` once it's stable
            match init_env() {
                Ok((env, song_history, recordings)) => {
                    let window = Window::new(&obj);
                    window.bind_models(&song_history, &recordings);
                    self.window.set(window.downgrade()).unwrap();
                    self.env.set((env, song_history, recordings)).unwrap();
                    window.present();
                }
                Err(err) => {
                    tracing::error!("Failed to setup db env: {:?}", err);

                    // TODO don't spawn a new window if one is already open
                    // or find a better solution in handling these errors
                    let err_window = DatabaseErrorWindow::new(&obj);
                    err_window.present();
                }
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

        fn shutdown(&self) {
            if let Some((env, _, _)) = self.env.get() {
                if let Err(err) = env.force_sync() {
                    tracing::error!("Failed to sync db env on shutdown: {:?}", err);
                }
            }

            if let Some((_, cache)) = self.session.get() {
                let now = Instant::now();
                cache.flush();
                cache.dump();
                tracing::debug!("Dumped soup cache in {:?}", now.elapsed());
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
        let (session, _) = self.imp().session.get_or_init(|| {
            let session = soup::Session::new();

            let cache_dir = {
                let mut path = glib::user_cache_dir();
                path.push("mousai/soup_cache");
                path
            };
            let cache_dir_str = cache_dir.to_str();

            if cache_dir_str.is_none() {
                tracing::warn!("Failed to convert cache dir to str");
            }

            let cache = soup::Cache::new(cache_dir_str, soup::CacheType::SingleUser);
            session.add_feature(&cache);

            let now = Instant::now();
            cache.load();
            tracing::debug!(path = ?cache.cache_dir(), "Loaded soup cache in {:?}", now.elapsed());

            (session, cache)
        });

        session
    }

    pub fn album_art_store(&self) -> &AlbumArtStore {
        self.imp()
            .album_art_store
            .get_or_init(|| AlbumArtStore::new(self.session()))
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

    pub fn quit(&self) {
        if let Some(window) = self.imp().window.get() {
            if let Some(window) = window.upgrade() {
                window.close();
            }
        }

        ApplicationExt::quit(self);
    }

    fn setup_gactions(&self) {
        let quit_action = gio::ActionEntry::builder("quit")
            .activate(|obj: &Self, _, _| {
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
        tracing::warn!("Failed to setup Mousai's inspector page. IOExtensionPoint `gtk-inspector-page` is likely not found");
    }
}

fn init_env() -> Result<(heed::Env, SongList, Recordings)> {
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

    let env = database::new_env()?;
    let song_history =
        SongList::load_from_env(env.clone()).context("Failed to load song history")?;
    let recordings = Recordings::load_from_env(env.clone())?;

    Ok((env, song_history, recordings))
}
