mod album_art;
mod audio_visualizer;
mod main_page;
mod song_bar;
mod song_cell;
mod song_page;
mod time_label;

use adw::subclass::prelude::*;
use gtk::{
    gdk, gio,
    glib::{self, clone},
    prelude::*,
    subclass::prelude::*,
};

use self::{main_page::MainPage, song_bar::SongBar, song_page::SongPage};
use crate::{
    config::PROFILE, core::PlaybackState, model::Song, song_player::SongPlayer, Application,
};

mod imp {
    use super::*;
    use gtk::CompositeTemplate;

    #[derive(Debug, Default, CompositeTemplate)]
    #[template(resource = "/io/github/seadve/Mousai/ui/window.ui")]
    pub struct Window {
        #[template_child]
        pub toast_overlay: TemplateChild<adw::ToastOverlay>,
        #[template_child]
        pub flap: TemplateChild<adw::Flap>,
        #[template_child]
        pub stack: TemplateChild<gtk::Stack>,
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
            klass.bind_template();
            klass.bind_template_instance_callbacks();

            klass.install_action("win.navigate-to-main-page", None, |obj, _, _| {
                let imp = obj.imp();
                imp.stack.set_visible_child(&imp.main_page.get());
            });

            klass.install_action("win.toggle-playback", None, |obj, _, _| {
                let player = obj.player();

                let res = if player.state() == PlaybackState::Playing {
                    player.pause()
                } else {
                    player.play()
                };

                if let Err(err) = res {
                    log::warn!("Failed to toggle playback: {err:?}");
                    obj.show_error(&err.to_string());
                }
            });

            klass.install_action("win.stop-playback", None, |obj, _, _| {
                if let Err(err) = obj.imp().player.set_song(None) {
                    log::warn!("Failed to stop player: {err:?}");
                }
            });

            klass.install_action("win.toggle-listen", None, |obj, _, _| {
                obj.imp().main_page.toggle_listen();
            });

            klass.install_action("win.toggle-search", None, |obj, _, _| {
                let search_bar = obj.imp().main_page.search_bar();
                search_bar.set_search_mode(!search_bar.is_search_mode());
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
            obj.update_toggle_playback_action();
            obj.update_main_page_actions();
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

    pub fn add_toast(&self, toast: &adw::Toast) {
        self.imp().toast_overlay.add_toast(toast);
    }

    pub fn show_error(&self, message: &str) {
        let toast = adw::Toast::builder()
            .title(message)
            .priority(adw::ToastPriority::High)
            .build();
        self.add_toast(&toast);
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

    fn update_toggle_playback_action(&self) {
        self.action_set_enabled("win.toggle-playback", self.player().song().is_some());
    }

    fn update_main_page_actions(&self) {
        let imp = self.imp();
        let is_main_page_visible =
            imp.stack.visible_child().as_ref() == Some(imp.main_page.get().upcast_ref());
        self.action_set_enabled("win.toggle-listen", is_main_page_visible);
        self.action_set_enabled("win.toggle-search", is_main_page_visible);
    }

    fn setup_signals(&self) {
        let imp = self.imp();

        imp.stack
            .connect_visible_child_notify(clone!(@weak self as obj => move |_| {
                obj.update_main_page_actions();
            }));

        imp.player
            .connect_song_notify(clone!(@weak self as obj => move |_| {
                obj.update_toggle_playback_action();
            }));

        imp.player
            .connect_error(clone!(@weak self as obj => move |_, error| {
                obj.show_error(&error.to_string());
            }));

        imp.main_page
            .connect_song_activated(clone!(@weak self as obj => move |_, song| {
                let imp = obj.imp();
                imp.song_page.set_song(Some(song.clone()));
                imp.stack.set_visible_child(&imp.song_page.get());
            }));

        imp.song_bar
            .connect_song_activated(clone!(@weak self as obj => move |_, song| {
                let imp = obj.imp();
                imp.song_page.set_song(Some(song.clone()));
                imp.stack.set_visible_child(&imp.song_page.get());
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
}

#[gtk::template_callbacks]
impl Window {
    #[template_callback]
    fn key_pressed(&self, keyval: gdk::Key, _keycode: u32, state: gdk::ModifierType) -> bool {
        if let Some(unicode) = keyval.to_unicode() {
            let search_bar = self.imp().main_page.search_bar();
            if !search_bar.is_search_mode()
                && keyval != gdk::Key::space
                && (state.contains(gdk::ModifierType::SHIFT_MASK) || state.is_empty())
                && unicode.is_alphanumeric()
            {
                if let Some(search_entry) = search_bar
                    .child()
                    .and_then(|child| child.downcast::<gtk::SearchEntry>().ok())
                {
                    search_entry.set_text(&unicode.to_string());
                    search_entry.set_position(1);
                    search_bar.set_search_mode(true);
                    return true;
                }

                log::error!("MainPage's SearchBar is expect to have a child of SearchEntry");
            }
        }
        false
    }
}
