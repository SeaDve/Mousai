use std::{cell::OnceCell, time::Instant};

use adw::{prelude::*, subclass::prelude::*};
use anyhow::{Context, Result};
use gettextrs::gettext;
use gtk::{
    gio,
    glib::{self, clone},
};
use soup::prelude::*;

use crate::{
    about,
    album_art::AlbumArtStore,
    config::{APP_ID, PKGDATADIR, PROFILE, VERSION},
    database::{self, EnvExt, Migrations},
    inspector_page::InspectorPage,
    preferences_dialog::PreferencesDialog,
    recognizer::Recordings,
    settings::Settings,
    song_list::SongList,
    window::Window,
};

mod imp {
    use super::*;

    #[derive(Default)]
    pub struct Application {
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

            let obj = self.obj();

            obj.window().present();
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

    /// Returns the global instance of `Application`.
    ///
    /// # Panics
    ///
    /// Panics if the app is not running or if this is called on a non-main thread.
    pub fn get() -> Self {
        debug_assert!(
            gtk::is_initialized_main_thread(),
            "application must only be accessed in the main thread"
        );

        gio::Application::default().unwrap().downcast().unwrap()
    }

    pub fn add_message_toast(&self, message: &str) {
        self.window().add_message_toast(message);
    }

    pub fn add_toast(&self, toast: adw::Toast) {
        self.window().add_toast(toast);
    }

    pub fn window(&self) -> Window {
        self.active_window().map_or_else(
            || {
                let imp = self.imp();

                let window = Window::new(self);

                match init_env() {
                    Ok((env, song_history, recordings)) => {
                        tracing::debug!("db env initialized");
                        window.bind_models(&song_history, &recordings);
                        imp.env.set((env, song_history, recordings)).unwrap();
                    }
                    Err(err) => {
                        tracing::error!("Failed to setup db env: {:?}", err);
                        self.present_database_error_dialog(&window);
                    }
                }

                window
            },
            |w| w.downcast().unwrap(),
        )
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
            .get_or_init(|| AlbumArtStore::new(self.session().clone()))
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
        if let Some(window) = self.active_window() {
            window.close();
        }

        ApplicationExt::quit(self);
    }

    fn present_database_error_dialog(&self, parent: &impl IsA<gtk::Widget>) {
        const QUIT_RESPONSE_ID: &str = "quit";

        let dialog = adw::AlertDialog::builder()
            .heading(gettext("Critical Database Error"))
            .body(gettext("Sorry, a critical database error has occurred. This is likely caused by a tampered or corrupted database. You can try clearing application data. However, this is not recommended and will delete all your songs and saved recordings.\n\nTo report this issue, please launch Mousai in the terminal to include the logs and submit the bug report to the <a href=\"https://github.com/SeaDve/Mousai/issues/\">issue page</a>"))
            .body_use_markup(true)
            .default_response(QUIT_RESPONSE_ID)
            .close_response(QUIT_RESPONSE_ID)
            .build();
        dialog.add_response(QUIT_RESPONSE_ID, &gettext("Quit"));
        dialog.set_response_appearance(QUIT_RESPONSE_ID, adw::ResponseAppearance::Suggested);
        dialog.connect_response(
            Some(QUIT_RESPONSE_ID),
            clone!(
                #[weak(rename_to = obj)]
                self,
                move |_, response| match response {
                    QUIT_RESPONSE_ID => obj.quit(),
                    _ => unreachable!(),
                }
            ),
        );
        dialog.present(Some(parent));
    }

    fn setup_gactions(&self) {
        let quit_action = gio::ActionEntry::builder("quit")
            .activate(|obj: &Self, _, _| {
                obj.quit();
            })
            .build();
        let show_preferences_action = gio::ActionEntry::builder("show-preferences")
            .activate(|obj: &Self, _, _| {
                let dialog = PreferencesDialog::new(obj.settings());
                dialog.present(Some(&obj.window()));
            })
            .build();
        let show_about_action = gio::ActionEntry::builder("show-about")
            .activate(|obj: &Self, _, _| {
                about::present_dialog(&obj.window());
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
                    song::Song,
                    uid::{Uid, UidCodec},
                };

                if let Some(db) = env.open_database::<SerdeBincode<Uid>, SerdeBincode<Song>>(
                    wtxn,
                    Some(SONG_LIST_DB_NAME),
                )? {
                    let new_items = db
                        .iter(wtxn)
                        .context("Failed to iter db")?
                        .collect::<Result<Vec<_>, _>>()
                        .context("Failed to collect items")?;

                    db.clear(wtxn)?;

                    let remapped_db = db.remap_key_type::<UidCodec>();

                    for (uid, song) in new_items {
                        remapped_db
                            .put(wtxn, &uid, &song)
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
