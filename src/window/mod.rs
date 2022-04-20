mod album_cover;
mod audio_visualizer;
mod external_link_tile;
mod history_view;
mod information_row;
mod playback_button;
mod recognizer_view;
mod song_bar;
mod song_page;
mod song_tile;
mod time_label;

use adw::{prelude::*, subclass::prelude::*};
use gettextrs::gettext;
use gtk::{
    gdk, gio,
    glib::{self, clone},
    subclass::prelude::*,
};
use once_cell::unsync::OnceCell;

use self::{history_view::HistoryView, recognizer_view::RecognizerView, song_bar::SongBar};
use crate::{
    config::PROFILE,
    model::SongList,
    player::{Player, PlayerState},
    recognizer::{Recognizer, RecognizerState},
    Application,
};

mod imp {
    use crate::spawn;

    use super::*;
    use gtk::CompositeTemplate;

    #[derive(Debug, Default, CompositeTemplate)]
    #[template(resource = "/io/github/seadve/Mousai/ui/window.ui")]
    pub struct Window {
        #[template_child]
        pub toast_overlay: TemplateChild<adw::ToastOverlay>,
        #[template_child]
        pub stack: TemplateChild<gtk::Stack>,
        #[template_child]
        pub main_view: TemplateChild<HistoryView>,
        #[template_child]
        pub recognizer_view: TemplateChild<RecognizerView>,
        #[template_child]
        pub song_bar_revealer: TemplateChild<gtk::Revealer>,
        #[template_child]
        pub song_bar: TemplateChild<SongBar>,

        pub recognizer: Recognizer,
        pub player: Player,
        pub history: OnceCell<SongList>,
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
                obj.imp().main_view.show_history();
            });

            klass.install_action("win.toggle-playback", None, |obj, _, _| {
                let player = obj.player();

                if player.state() == PlayerState::Playing {
                    player.pause();
                } else {
                    player.play();
                };
            });

            klass.install_action("win.stop-playback", None, |obj, _, _| {
                if let Err(err) = obj.imp().player.stop() {
                    log::warn!("Failed to stop player: {err:?}");
                }
            });

            klass.install_action("win.toggle-listen", None, |obj, _, _| {
                spawn!(clone!(@weak obj => async move {
                    if let Err(err) = obj.imp().player.stop() {
                        log::warn!("Failed to stop player before toggling listen: {err:?}");
                    }
                    if let Err(err) = obj.imp().recognizer.toggle_recognize().await {
                        log::error!("Failed to toggle recognize: {:?}", err);
                        obj.show_error(&err.to_string());
                    }
                }));
            });

            klass.install_action("win.toggle-search", None, |obj, _, _| {
                let search_bar = obj.imp().main_view.search_bar();
                search_bar.set_search_mode(!search_bar.is_search_mode());
            });

            klass.install_action("undo-remove-toast.dismiss", None, |obj, _, _| {
                obj.imp().main_view.undo_remove();
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
            self.main_view.bind_player(&self.player);
            self.main_view.bind_song_list(obj.history());
            self.recognizer_view.bind_recognizer(&self.recognizer);

            obj.setup_signals();

            obj.load_window_size();
            obj.update_song_bar_revealer();
            obj.update_stack();
            obj.update_toggle_playback_action();
            obj.update_toggle_listen_action();
            obj.update_toggle_search_action();
        }
    }

    impl WidgetImpl for Window {}
    impl WindowImpl for Window {
        fn close_request(&self, obj: &Self::Type) -> gtk::Inhibit {
            if let Err(err) = obj.save_window_size() {
                log::warn!("Failed to save window state, {:?}", &err);
            }

            if let Err(err) = obj.history().save_to_settings() {
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

    pub fn player(&self) -> Player {
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

    fn history(&self) -> &SongList {
        self.imp().history.get_or_init(|| {
            SongList::load_from_settings().unwrap_or_else(|err| {
                log::error!("Failed to load SongList from settings: {err:?}");
                self.show_error(&gettext("Failed to load history"));
                SongList::default()
            })
        })
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

    fn update_toggle_listen_action(&self) {
        match self.imp().recognizer.state() {
            RecognizerState::Null | RecognizerState::Listening => {
                self.action_set_enabled("win.toggle-listen", true);
            }
            RecognizerState::Recognizing => {
                // TODO: Fix cancellation during recognizing state in recognizer.rs and remove this
                self.action_set_enabled("win.toggle-listen", false);
            }
        }
    }

    fn update_toggle_playback_action(&self) {
        self.action_set_enabled("win.toggle-playback", self.player().song().is_some());
    }

    fn update_toggle_search_action(&self) {
        let imp = self.imp();
        let is_main_page_visible =
            imp.stack.visible_child().as_ref() == Some(imp.main_view.get().upcast_ref());
        self.action_set_enabled("win.toggle-search", is_main_page_visible);
    }

    fn update_stack(&self) {
        let imp = self.imp();

        match imp.recognizer.state() {
            RecognizerState::Listening | RecognizerState::Recognizing => {
                imp.stack.set_visible_child(&imp.recognizer_view.get());
            }
            RecognizerState::Null => {
                imp.stack.set_visible_child(&imp.main_view.get());
            }
        }
    }

    fn setup_signals(&self) {
        let imp = self.imp();

        imp.player
            .connect_song_notify(clone!(@weak self as obj => move |_| {
                obj.update_toggle_playback_action();
                obj.update_song_bar_revealer();
            }));

        imp.player
            .connect_error(clone!(@weak self as obj => move |_, error| {
                obj.show_error(&error.to_string());
            }));

        imp.recognizer
            .connect_state_notify(clone!(@weak self as obj => move |_| {
                obj.update_toggle_listen_action();
                obj.update_stack();
            }));

        imp.recognizer
            .connect_song_recognized(clone!(@weak self as obj => move |_, song| {
                obj.history().append(song.clone());
                obj.imp().main_view.show_song(song);
            }));

        imp.song_bar
            .connect_song_activated(clone!(@weak self as obj => move |_, song| {
                obj.imp().main_view.show_song(song);
            }));

        imp.main_view
            .connect_selection_mode_notify(clone!(@weak self as obj => move |_| {
                obj.update_song_bar_revealer();
            }));

        imp.stack
            .connect_visible_child_notify(clone!(@weak self as obj => move |_| {
                obj.update_toggle_search_action();
            }));

        self.history()
            .connect_removed(clone!(@weak self as obj => move |_, song| {
                let player = obj.player();
                if player.is_active_song(song) {
                    if let Err(err) = player.stop() {
                        log::warn!("Failed to stop player while deleting the active song: {err:?}");
                    }
                }
            }));
    }

    fn update_song_bar_revealer(&self) {
        let imp = self.imp();
        imp.song_bar_revealer
            .set_reveal_child(self.player().song().is_some() && !imp.main_view.is_selection_mode());
    }
}

#[gtk::template_callbacks]
impl Window {
    #[template_callback]
    fn key_pressed(&self, keyval: gdk::Key, _keycode: u32, state: gdk::ModifierType) -> bool {
        let imp = self.imp();
        let search_bar = imp.main_view.search_bar();

        if keyval == gdk::Key::Escape
            && state == gdk::ModifierType::empty()
            && search_bar.is_search_mode()
            && !imp.main_view.is_on_song_page()
        {
            search_bar.set_search_mode(false);
            return true;
        }

        if keyval == gdk::Key::Escape
            && state == gdk::ModifierType::empty()
            && imp.main_view.is_selection_mode()
            && !imp.main_view.is_on_song_page()
        {
            imp.main_view.stop_selection_mode();
            return true;
        }

        if let Some(unicode) = keyval.to_unicode() {
            if !search_bar.is_search_mode()
                && keyval != gdk::Key::space
                && ((state == gdk::ModifierType::SHIFT_MASK) || state.is_empty())
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
