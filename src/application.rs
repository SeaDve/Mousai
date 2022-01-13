use adw::subclass::prelude::*;
use gettextrs::gettext;
use gtk::{
    gio,
    glib::{self, clone},
    prelude::*,
    subclass::prelude::*,
};

use crate::{
    config::{APP_ID, PKGDATADIR, PROFILE, VERSION},
    window::Window,
};

mod imp {
    use super::*;
    use glib::WeakRef;
    use once_cell::sync::OnceCell;

    #[derive(Debug)]
    pub struct Application {
        pub window: OnceCell<WeakRef<Window>>,
        pub settings: gio::Settings,
    }

    impl Default for Application {
        fn default() -> Self {
            Self {
                window: OnceCell::new(),
                settings: gio::Settings::new(APP_ID),
            }
        }
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

            if let Some(window) = self.window.get() {
                let window = window.upgrade().unwrap();
                window.show();
                window.present();
                return;
            }

            ///////

            let mut songs = Vec::new();

            for i in 0..100 {
                use rand::Rng;

                let rand_title: String = rand::thread_rng()
                    .sample_iter(&rand::distributions::Alphanumeric)
                    .take(rand::thread_rng().gen_range(5..10))
                    .map(char::from)
                    .collect();

                let rand_artist: String = rand::thread_rng()
                    .sample_iter(&rand::distributions::Alphanumeric)
                    .take(rand::thread_rng().gen_range(10..15))
                    .map(char::from)
                    .collect();

                songs.push(crate::model::Song::new(
                    &rand_title,
                    &rand_artist,
                    &i.to_string(),
                ));
            }

            let history = crate::model::SongList::new();
            history.append_many(songs);

            ///////

            let window = Window::new(obj, &history);
            self.window
                .set(window.downgrade())
                .expect("Window already set.");

            obj.main_window().present();
        }

        fn startup(&self, obj: &Self::Type) {
            self.parent_startup(obj);

            gtk::Window::set_default_icon_name(APP_ID);

            obj.setup_gactions();
            obj.setup_accels();
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
        .expect("Application initialization failed...")
    }

    pub fn settings(&self) -> gio::Settings {
        let imp = imp::Application::from_instance(self);
        imp.settings.clone()
    }

    pub fn run(&self) {
        log::info!("Mousai ({})", APP_ID);
        log::info!("Version: {} ({})", VERSION, PROFILE);
        log::info!("Datadir: {}", PKGDATADIR);

        ApplicationExtManual::run(self);
    }

    fn main_window(&self) -> Window {
        let imp = imp::Application::from_instance(self);
        imp.window.get().unwrap().upgrade().unwrap()
    }

    fn setup_gactions(&self) {
        let action_quit = gio::SimpleAction::new("quit", None);
        action_quit.connect_activate(clone!(@weak self as obj => move |_, _| {
            obj.main_window().close();
            obj.quit();
        }));
        self.add_action(&action_quit);

        let action_about = gio::SimpleAction::new("about", None);
        action_about.connect_activate(clone!(@weak self as obj => move |_, _| {
            obj.show_about_dialog();
        }));
        self.add_action(&action_about);
    }

    fn setup_accels(&self) {
        self.set_accels_for_action("app.quit", &["<primary>q"]);
    }

    fn show_about_dialog(&self) {
        let dialog = gtk::AboutDialog::builder()
            .transient_for(&self.main_window())
            .modal(true)
            .comments(&gettext("Identify any songs in seconds"))
            .version(VERSION)
            .logo_icon_name(APP_ID)
            .authors(vec!["Dave Patrick".into()])
            // Translators: Replace "translator-credits" with your names. Put a comma between.
            .translator_credits(&gettext("translator-credits"))
            .copyright(&gettext("Copyright 2022 Dave Patrick"))
            .license_type(gtk::License::Gpl30)
            .website("https://github.com/SeaDve/Mousai")
            .website_label(&gettext("GitHub"))
            .build();

        dialog.show();
    }
}

impl Default for Application {
    fn default() -> Self {
        gio::Application::default().unwrap().downcast().unwrap()
    }
}
