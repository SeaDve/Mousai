mod album_cover;
mod external_link_tile;
mod history_view;
mod information_row;
mod playback_button;
mod recognizer_view;
mod song_bar;
mod song_page;
mod song_tile;
mod time_label;
mod waveform;

use adw::{prelude::*, subclass::prelude::*};
use anyhow::{Error, Result};
use gtk::{
    gdk, gio,
    glib::{self, clone},
};
use once_cell::unsync::OnceCell;

use std::cell::Cell;

use self::{history_view::HistoryView, recognizer_view::RecognizerView, song_bar::SongBar};
use crate::{
    config::PROFILE,
    core::DateTime,
    model::SongList,
    player::{Player, PlayerState},
    recognizer::{Recognizer, RecognizerState},
    utils, Application,
};

// 570 is perfect to prevent three columns history grid view on narrow mode.
const NARROW_ADAPTIVE_MODE_THRESHOLD: i32 = 570;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, glib::Enum)]
#[enum_type(name = "MsaiAdaptiveMode")]
pub enum AdaptiveMode {
    #[default]
    Normal,
    Narrow,
}

mod imp {
    use super::*;
    use gtk::CompositeTemplate;
    use once_cell::sync::Lazy;

    #[derive(Debug, Default, CompositeTemplate)]
    #[template(resource = "/io/github/seadve/Mousai/ui/window.ui")]
    pub struct Window {
        #[template_child]
        pub(super) toast_overlay: TemplateChild<adw::ToastOverlay>,
        #[template_child]
        pub(super) stack: TemplateChild<gtk::Stack>,
        #[template_child]
        pub(super) main_view: TemplateChild<HistoryView>,
        #[template_child]
        pub(super) recognizer_view: TemplateChild<RecognizerView>,
        #[template_child]
        pub(super) song_bar_revealer: TemplateChild<gtk::Revealer>,
        #[template_child]
        pub(super) song_bar: TemplateChild<SongBar>,

        pub(super) adaptive_mode: Cell<AdaptiveMode>,

        pub(super) recognizer: Recognizer,
        pub(super) player: Player,
        pub(super) history: OnceCell<SongList>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Window {
        const NAME: &'static str = "MsaiWindow";
        type Type = super::Window;
        type ParentType = adw::ApplicationWindow;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
            klass.bind_template_instance_callbacks();

            klass.install_action("win.navigate-back", None, |obj, _, _| {
                obj.imp().main_view.pop_song_page();
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
                obj.imp().player.clear_song();
            });

            klass.install_action_async("win.toggle-listen", None, |obj, _, _| async move {
                obj.imp().player.clear_song();

                if let Err(err) = obj.imp().recognizer.toggle_recognize().await {
                    tracing::error!("{:?}", err);
                    utils::app_instance().present_error(&err);
                }
            });

            klass.install_action("win.toggle-search", None, |obj, _, _| {
                let search_bar = obj.imp().main_view.search_bar();
                search_bar.set_search_mode(!search_bar.is_search_mode());
            });

            klass.install_action("undo-remove-toast.undo", None, |obj, _, _| {
                obj.imp().main_view.undo_remove();
            });
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for Window {
        fn properties() -> &'static [glib::ParamSpec] {
            static PROPERTIES: Lazy<Vec<glib::ParamSpec>> = Lazy::new(|| {
                vec![
                    // Current adapative mode
                    glib::ParamSpecEnum::builder("adaptive-mode", AdaptiveMode::default())
                        .read_only()
                        .build(),
                ]
            });

            PROPERTIES.as_ref()
        }

        fn property(&self, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
            let obj = self.instance();

            match pspec.name() {
                "adaptive-mode" => obj.adaptive_mode().to_value(),
                _ => unreachable!(),
            }
        }

        fn constructed(&self) {
            self.parent_constructed();

            let obj = self.instance();

            let preferred_audio_source_action = utils::app_instance()
                .settings()
                .create_preferred_audio_source_action();
            obj.add_action(&preferred_audio_source_action);

            if PROFILE == "Devel" {
                obj.add_css_class("devel");
            }

            self.song_bar.bind_player(&self.player);
            self.main_view.bind_player(&self.player);
            self.main_view.bind_song_list(obj.history());
            self.recognizer_view.bind_recognizer(&self.recognizer);

            self.main_view
                .search_bar()
                .set_key_capture_widget(Some(obj.upcast_ref::<gtk::Widget>()));

            obj.bind_property("adaptive-mode", &self.main_view.get(), "adaptive-mode")
                .flags(glib::BindingFlags::SYNC_CREATE)
                .build();

            obj.setup_signals();

            obj.load_window_size();
            obj.update_song_bar_revealer();
            obj.update_stack();
            obj.update_toggle_playback_action();
            obj.update_toggle_search_action();
        }
    }

    impl WidgetImpl for Window {
        fn realize(&self) {
            self.parent_realize();

            let obj = self.instance();

            obj.surface()
                .connect_width_notify(clone!(@weak obj => move |_| {
                    obj.update_adaptive_mode();
                }));

            obj.update_adaptive_mode();
        }
    }

    impl WindowImpl for Window {
        fn close_request(&self) -> gtk::Inhibit {
            let obj = self.instance();

            if let Err(err) = obj.save_window_size() {
                tracing::warn!("Failed to save window state, {:?}", &err);
            }

            if let Err(err) = obj.history().save_to_settings() {
                tracing::error!("Failed to save history: {:?}", err);
            }

            self.parent_close_request()
        }
    }

    impl ApplicationWindowImpl for Window {}
    impl AdwApplicationWindowImpl for Window {}
}

glib::wrapper! {
    pub struct Window(ObjectSubclass<imp::Window>)
        @extends gtk::Widget, gtk::Window, gtk::ApplicationWindow, adw::ApplicationWindow,
        @implements gio::ActionMap, gio::ActionGroup, gtk::Native, gtk::Root;
}

impl Window {
    pub fn new(app: &Application) -> Self {
        glib::Object::new(&[("application", app)])
    }

    pub fn player(&self) -> Player {
        self.imp().player.clone()
    }

    pub fn add_toast(&self, toast: &adw::Toast) {
        self.imp().toast_overlay.add_toast(toast);
    }

    pub fn connect_adaptive_mode_notify<F>(&self, f: F) -> glib::SignalHandlerId
    where
        F: Fn(&Self) + 'static,
    {
        self.connect_notify_local(Some("adaptive-mode"), move |obj, _| f(obj))
    }

    pub fn adaptive_mode(&self) -> AdaptiveMode {
        self.imp().adaptive_mode.get()
    }

    fn history(&self) -> &SongList {
        self.imp().history.get_or_init(|| {
            SongList::load_from_settings().unwrap_or_else(|err| {
                let err = err.context("Failed to load SongList from settings");

                tracing::error!("{:?}", err);
                tracing::debug!("Using empty SongList instead");
                utils::app_instance().present_error(&err);

                SongList::default()
            })
        })
    }

    fn load_window_size(&self) {
        let settings = utils::app_instance().settings();

        self.set_default_size(settings.window_width(), settings.window_height());

        if settings.is_maximized() {
            self.maximize();
        }
    }

    fn save_window_size(&self) -> Result<()> {
        let settings = utils::app_instance().settings();

        let (width, height) = self.default_size();

        settings.try_set_window_width(width)?;
        settings.try_set_window_height(height)?;

        settings.try_set_is_maximized(self.is_maximized())?;

        Ok(())
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

    fn update_adaptive_mode(&self) {
        let width = self.surface().width();

        let adaptive_mode = if width < NARROW_ADAPTIVE_MODE_THRESHOLD {
            AdaptiveMode::Narrow
        } else {
            AdaptiveMode::Normal
        };

        if adaptive_mode == self.adaptive_mode() {
            return;
        }

        self.imp().adaptive_mode.set(adaptive_mode);
        self.notify("adaptive-mode");
    }

    fn setup_signals(&self) {
        let imp = self.imp();

        imp.player
            .connect_song_notify(clone!(@weak self as obj => move |_| {
                obj.update_toggle_playback_action();
                obj.update_song_bar_revealer();
            }));

        imp.player.connect_error(|_, err| {
            let err = Error::from(err.clone()).context("Player error");
            utils::app_instance().add_toast_error(&err);
        });

        imp.recognizer
            .connect_state_notify(clone!(@weak self as obj => move |_| {
                obj.update_stack();
            }));

        imp.recognizer
            .connect_song_recognized(clone!(@weak self as obj => move |_, song| {
                let history = obj.history();
                let contains_song = history.contains(&song.id());

                if contains_song {
                    song.set_last_heard(DateTime::now());
                }

                // We also need to emit items_changed to update sort list model
                // order, and update to new properties if any.
                let is_appended = history.append(song.clone());

                if contains_song == is_appended {
                    tracing::error!("History already contains song, but it was still successfully appended");
                }

                let main_view = &obj.imp().main_view;
                main_view.push_song_page(song);
                main_view.scroll_to_top();
            }));

        imp.song_bar
            .connect_activated(clone!(@weak self as obj => move |_, song| {
                obj.imp().main_view.push_song_page(song);
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
                    player.clear_song();
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
            && !imp.main_view.is_on_song_page()
        {
            if search_bar.is_search_mode() {
                search_bar.set_search_mode(false);
                return true;
            }

            if imp.main_view.is_selection_mode() {
                imp.main_view.stop_selection_mode();
                return true;
            }
        }

        false
    }
}
