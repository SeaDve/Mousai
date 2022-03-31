mod album_art;
mod audio_visualizer;
mod main_page;
mod song_bar;
mod song_cell;
mod song_page;
mod time_label;

use adw::subclass::prelude::*;
use gtk::{
    gio,
    glib::{self, clone},
    prelude::*,
    subclass::prelude::*,
};

use self::{main_page::MainPage, song_bar::SongBar, song_page::SongPage};
use crate::{config::PROFILE, model::Song, song_player::SongPlayer, Application};

mod imp {
    use super::*;
    use gtk::CompositeTemplate;

    #[derive(Debug, Default, CompositeTemplate)]
    #[template(resource = "/io/github/seadve/Mousai/ui/window.ui")]
    pub struct Window {
        #[template_child]
        pub flap: TemplateChild<adw::Flap>,
        #[template_child]
        pub main_stack: TemplateChild<gtk::Stack>,
        #[template_child]
        pub main_page: TemplateChild<MainPage>,
        #[template_child]
        pub song_page: TemplateChild<SongPage>,
        #[template_child]
        pub song_bar: TemplateChild<SongBar>,

        pub player: SongPlayer,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Window {
        const NAME: &'static str = "MsaiWindow";
        type Type = super::Window;
        type ParentType = adw::ApplicationWindow;

        fn class_init(klass: &mut Self::Class) {
            Self::bind_template(klass);

            klass.install_action("win.navigate-to-main-page", None, move |obj, _, _| {
                let imp = obj.imp();
                imp.main_stack.set_visible_child(&imp.main_page.get());
            });
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for Window {
        fn constructed(&self, obj: &Self::Type) {
            self.parent_constructed(obj);

            let preferred_audio_source_action = Application::default()
                .settings()
                .create_action("preferred-audio-source");
            obj.add_action(&preferred_audio_source_action);

            if PROFILE == "Devel" {
                obj.add_css_class("devel");
            }

            self.song_bar.bind_player(&self.player);

            obj.setup_signals();
            obj.setup_bindings();

            obj.load_window_size();
        }
    }

    impl WidgetImpl for Window {}
    impl WindowImpl for Window {
        fn close_request(&self, obj: &Self::Type) -> gtk::Inhibit {
            if let Err(err) = obj.save_window_size() {
                log::warn!("Failed to save window state, {:?}", &err);
            }

            if let Err(err) = self.main_page.save_history() {
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
        @implements gio::ActionMap, gio::ActionGroup, gtk::Root;
}

impl Window {
    pub fn new(app: &Application) -> Self {
        glib::Object::new(&[("application", app)]).expect("Failed to create Window")
    }

    pub fn player(&self) -> SongPlayer {
        self.imp().player.clone()
    }

    fn setup_signals(&self) {
        let imp = self.imp();

        imp.main_page
            .connect_song_activated(clone!(@weak self as obj => move |_, song| {
                let imp = obj.imp();
                imp.song_page.set_song(Some(song.clone()));
                imp.main_stack.set_visible_child(&imp.song_page.get());
            }));

        imp.song_bar
            .connect_song_activated(clone!(@weak self as obj => move |_, song| {
                let imp = obj.imp();
                imp.song_page.set_song(Some(song.clone()));
                imp.main_stack.set_visible_child(&imp.song_page.get());
            }));
    }

    fn setup_bindings(&self) {
        let imp = self.imp();
        imp.player
            .bind_property("song", &imp.flap.get(), "reveal-flap")
            .transform_to(|_, value| {
                let song: Option<Song> = value.get().unwrap();
                Some(song.is_some().to_value())
            })
            .flags(glib::BindingFlags::SYNC_CREATE)
            .build();
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
}
