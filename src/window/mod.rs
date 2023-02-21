mod album_cover;
mod external_link_tile;
mod history_view;
mod information_row;
mod playback_button;
mod progress_icon;
mod recognized_page;
mod recognized_page_tile;
mod recognizer_status;
mod recognizer_view;
mod song_bar;
mod song_page;
mod song_tile;
mod waveform;

use adw::{prelude::*, subclass::prelude::*};
use anyhow::{Error, Result};
use gettextrs::gettext;
use gtk::{
    gdk, gio,
    glib::{self, clone},
};
use once_cell::unsync::OnceCell;

use std::cell::Cell;

use self::{history_view::HistoryView, recognizer_view::RecognizerView, song_bar::SongBar};
use crate::{
    config::PROFILE,
    model::SongList,
    player::{Player, PlayerState},
    recognizer::{Recognizer, RecognizerState},
    utils, Application,
};

// 570 is just right to prevent three columns history grid view on narrow mode.
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

    #[derive(Debug, Default, glib::Properties, gtk::CompositeTemplate)]
    #[properties(wrapper_type = super::Window)]
    #[template(resource = "/io/github/seadve/Mousai/ui/window.ui")]
    pub struct Window {
        #[property(get, builder(AdaptiveMode::default()))]
        pub(super) adaptive_mode: Cell<AdaptiveMode>,

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
                obj.imp().main_view.navigate_back();
            });

            klass.install_action("win.navigate-forward", None, |obj, _, _| {
                obj.imp().main_view.navigate_forward();
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
                obj.imp().player.set_song(None);
            });

            klass.install_action_async("win.toggle-listen", None, |obj, _, _| async move {
                obj.imp().player.set_song(None);

                if let Err(err) = obj.imp().recognizer.toggle_recognize().await {
                    tracing::error!("{:?}", err);
                    utils::app_instance().present_error(&err);
                }
            });

            klass.install_action("win.toggle-search", None, |obj, _, _| {
                let search_bar = obj.imp().main_view.search_bar();
                search_bar.set_search_mode(!search_bar.is_search_mode());
            });
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for Window {
        crate::derived_properties!();

        fn constructed(&self) {
            self.parent_constructed();

            let obj = self.obj();

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
            self.main_view.bind_recognizer(&self.recognizer);
            self.recognizer_view.bind_recognizer(&self.recognizer);

            self.main_view
                .search_bar()
                .set_key_capture_widget(Some(obj.upcast_ref::<gtk::Widget>()));

            obj.bind_property("adaptive-mode", &self.main_view.get(), "adaptive-mode")
                .sync_create()
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

            let obj = self.obj();

            obj.surface()
                .connect_width_notify(clone!(@weak obj => move |_| {
                    obj.update_adaptive_mode();
                }));

            obj.update_adaptive_mode();
        }
    }

    impl WindowImpl for Window {
        fn close_request(&self) -> gtk::Inhibit {
            let obj = self.obj();

            if let Err(err) = obj.save_window_size() {
                tracing::warn!("Failed to save window state, {:?}", &err);
            }

            if let Err(err) = obj.history().save_to_settings() {
                tracing::error!("Failed to save history: {:?}", err);
            }

            if let Err(err) = self.recognizer.save_saved_recordings() {
                tracing::error!("Failed to save saved recordings: {:?}", err);
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
        glib::Object::builder().property("application", app).build()
    }

    pub fn player(&self) -> Player {
        self.imp().player.clone()
    }

    pub fn add_toast(&self, toast: adw::Toast) {
        self.imp().toast_overlay.add_toast(toast);
    }

    fn history(&self) -> &SongList {
        self.imp().history.get_or_init(|| {
            SongList::load_from_settings().unwrap_or_else(|err| {
                let err = err.context("Failed to load SongList from settings");

                tracing::error!("{:?}", err);
                tracing::debug!("Using empty SongList instead");

                // FIXME this causes weird behavior since the first call
                // to history() is in constructed() where the window is not
                // yet fully initialized. Thus, when present error, which needed
                // the window, is called, it causes a weirdness. A fix is to defer
                // loading the history.
                utils::app_instance().present_error(&err);

                SongList::default()
            })
        })
    }

    fn load_window_size(&self) {
        let settings = utils::app_instance().settings();

        self.set_default_size(settings.window_width(), settings.window_height());

        if settings.window_maximized() {
            self.maximize();
        }
    }

    fn save_window_size(&self) -> Result<()> {
        let settings = utils::app_instance().settings();

        let (width, height) = self.default_size();

        settings.try_set_window_width(width)?;
        settings.try_set_window_height(height)?;

        settings.try_set_window_maximized(self.is_maximized())?;

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

        // FIXME make less hacky
        let adaptive_mode = if width < NARROW_ADAPTIVE_MODE_THRESHOLD {
            AdaptiveMode::Narrow
        } else {
            AdaptiveMode::Normal
        };

        if adaptive_mode == self.adaptive_mode() {
            return;
        }

        self.imp().adaptive_mode.set(adaptive_mode);
        self.notify_adaptive_mode();
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

                if !history.contains(&song.id()) {
                    song.set_is_newly_heard(true);
                }

                history.append(song.clone());

                let main_view = obj.imp().main_view.get();
                main_view.insert_song_page(song);
                main_view.scroll_to_top();
            }));

        imp.recognizer
            .connect_recording_saved(clone!(@weak self as obj => move |_| {
                let dialog = adw::MessageDialog::builder()
                    .heading(gettext("Recording saved"))
                    .body(gettext("The result will be available when you're back online."))
                    .default_response("ok")
                    .transient_for(&obj)
                    .modal(true)
                    .build();
                dialog.add_response("ok", &gettext("Ok, got it"));
                dialog.present();
            }));

        imp.song_bar
            .connect_activated(clone!(@weak self as obj => move |_, song| {
                obj.imp().main_view.insert_song_page(song);
            }));

        imp.main_view.connect_is_selection_mode_active_notify(
            clone!(@weak self as obj => move |_| {
                obj.update_song_bar_revealer();
            }),
        );

        imp.stack
            .connect_visible_child_notify(clone!(@weak self as obj => move |_| {
                obj.update_toggle_search_action();
            }));

        self.history()
            .connect_removed(clone!(@weak self as obj => move |_, song| {
                let player = obj.player();
                if player.is_active_song(song) {
                    player.set_song(None);
                }
            }));
    }

    fn update_song_bar_revealer(&self) {
        let imp = self.imp();
        imp.song_bar_revealer.set_reveal_child(
            self.player().song().is_some() && !imp.main_view.is_selection_mode_active(),
        );
    }
}

#[gtk::template_callbacks]
impl Window {
    #[template_callback]
    fn key_pressed(&self, keyval: gdk::Key, _keycode: u32, state: gdk::ModifierType) -> bool {
        let imp = self.imp();

        if keyval == gdk::Key::Escape
            && state == gdk::ModifierType::empty()
            && imp.main_view.is_on_leaflet_main_page()
        {
            let search_bar = imp.main_view.search_bar();
            if search_bar.is_search_mode() {
                search_bar.set_search_mode(false);
                return true;
            }

            if imp.main_view.is_selection_mode_active() {
                imp.main_view.stop_selection_mode();
                return true;
            }
        }

        false
    }
}
