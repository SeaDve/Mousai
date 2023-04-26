mod album_cover;
mod external_link_tile;
mod history_view;
mod information_row;
mod playback_button;
mod progress_icon;
mod recognized_page;
mod recognized_page_tile;
mod recognizer_status;
mod crossfade_paintable;
mod recognizer_view;
mod song_bar;
mod song_page;
mod song_tile;
mod waveform;

use adw::{prelude::*, subclass::prelude::*};
use anyhow::Result;
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
    model::{Song, SongList},
    player::{Player, PlayerState},
    preferences_window::PreferencesWindow,
    recognizer::{RecognizeError, RecognizeErrorKind, Recognizer, RecognizerState, Recordings},
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

        pub(super) player: Player,
        pub(super) recognizer: Recognizer,
        pub(super) song_history: OnceCell<SongList>,
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
                let imp = obj.imp();

                if imp.player.state() == PlayerState::Playing {
                    imp.player.pause();
                } else {
                    imp.player.play();
                };
            });

            klass.install_action("win.stop-playback", None, |obj, _, _| {
                obj.imp().player.set_song(Song::NONE);
            });

            klass.install_action_async("win.toggle-recognize", None, |obj, _, _| async move {
                let imp = obj.imp();

                imp.player.set_song(Song::NONE);

                if let Err(err) = imp.recognizer.toggle_recognize().await {
                    tracing::error!("{:?} (dbg: {:#?})", err, err);

                    if let Some(recognize_error) = err.downcast_ref::<RecognizeError>() {
                        obj.present_recognize_error(recognize_error);
                    } else {
                        obj.add_message_toast(&err.to_string());
                    }
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

            if PROFILE == "Devel" {
                obj.add_css_class("devel");
            }

            self.song_bar.bind_player(&self.player);
            self.main_view.bind_player(&self.player);

            self.main_view
                .search_bar()
                .set_key_capture_widget(Some(obj.as_ref()));

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
    pub fn new(application: &Application) -> Self {
        glib::Object::builder()
            .property("application", application)
            .build()
    }

    pub fn bind_models(&self, song_history: &SongList, recordings: &Recordings) {
        let imp = self.imp();

        imp.song_history
            .set(song_history.clone())
            .expect("song history must be bound only once");

        song_history.connect_items_changed(
            clone!(@weak self as obj => move |history, _index, removed, _added| {
                if removed == 0 {
                    return;
                }

                let imp = obj.imp();
                if let Some(active_song) = imp.player.song() {
                    if !history.contains(active_song.id_ref()) {
                        imp.player.set_song(Song::NONE);
                    }
                }
            }),
        );

        imp.main_view.bind_song_list(song_history);
        imp.recognizer.bind_saved_recordings(recordings);

        // Recognizer must have saved recordings first
        imp.main_view.bind_recognizer(&imp.recognizer);
        imp.recognizer_view.bind_recognizer(&imp.recognizer);

        imp.recognizer
            .connect_state_notify(clone!(@weak self as obj => move |_| {
                obj.update_stack();
            }));
        imp.recognizer
            .connect_song_recognized(clone!(@weak self as obj => move |_, song| {
                let history = obj.song_history();

                // If the song is not found in the history, set it as newly heard
                // (That's why an always true value is used after `or`). If it is in the
                // history and it was newly heard, pass that state to the new value.
                if history
                    .get(song.id_ref())
                    .map_or(true, |prev| prev.is_newly_heard())
                {
                    song.set_is_newly_heard(true);
                }

                if let Err(err) = history.insert(song.clone()) {
                    tracing::error!("Failed to insert song to history: {:?}", err);
                    obj.add_message_toast(&gettext("Failed to insert song to history"));
                    return;
                }

                let main_view = obj.imp().main_view.get();
                main_view.insert_song_page(song);
                main_view.scroll_to_top();
            }));
        imp.recognizer
            .connect_recording_saved(clone!(@weak self as obj => move |_, message| {
                let dialog = adw::MessageDialog::builder()
                    .heading(gettext("Recording saved"))
                    .body(message)
                    .default_response("ok")
                    .transient_for(&obj)
                    .modal(true)
                    .build();
                dialog.add_response("ok", &gettext("Ok, got it"));
                dialog.present();
            }));
    }

    pub fn add_toast(&self, toast: adw::Toast) {
        self.imp().toast_overlay.add_toast(toast);
    }

    pub fn add_message_toast(&self, message: &str) {
        let toast = adw::Toast::new(message);
        self.add_toast(toast);
    }

    fn song_history(&self) -> &SongList {
        self.imp()
            .song_history
            .get()
            .expect("song history must be bound")
    }

    fn present_recognize_error(&self, err: &RecognizeError) {
        let dialog = adw::MessageDialog::builder()
            .transient_for(self)
            .modal(true)
            .heading(err.title())
            .build();

        match err.kind() {
            RecognizeErrorKind::InvalidToken | RecognizeErrorKind::TokenLimitReached => {
                const OPEN_RESPONSE_ID: &str = "open";
                const NO_RESPONSE_ID: &str = "no";

                match err.kind() {
                    RecognizeErrorKind::InvalidToken => {
                        dialog.set_body(&gettext(
                            "Open preferences and try setting a different token.",
                        ));
                    }
                    RecognizeErrorKind::TokenLimitReached => {
                        dialog.set_body(&gettext(
                            "Wait until the limit is reset or open preferences and try setting a different token.",
                        ));
                    }
                    _ => unreachable!(),
                }

                dialog.add_response(OPEN_RESPONSE_ID, &gettext("Open Preferences"));
                dialog
                    .set_response_appearance(OPEN_RESPONSE_ID, adw::ResponseAppearance::Suggested);
                dialog.set_default_response(Some(OPEN_RESPONSE_ID));

                dialog.add_response(NO_RESPONSE_ID, &gettext("No, Thanks"));

                dialog.connect_response(
                    Some(OPEN_RESPONSE_ID),
                    clone!(@weak self as obj => move |_, id| {
                        debug_assert_eq!(id, OPEN_RESPONSE_ID);

                        let window = PreferencesWindow::new(utils::app_instance().settings());
                        window.set_transient_for(Some(&obj));
                        window.present();

                        let is_focused = window.focus_aud_d_api_token_row();
                        debug_assert!(is_focused);
                    }),
                );
            }
            RecognizeErrorKind::OtherPermanent | RecognizeErrorKind::Fingerprint => {
                const OPEN_RESPONSE_ID: &str = "open";
                const NO_RESPONSE_ID: &str = "no";

                dialog.set_body(&gettext(
                    "Please open an issue on GitHub and provide the necessary information.",
                ));

                dialog.add_response(OPEN_RESPONSE_ID, &gettext("Open an Issue"));
                dialog
                    .set_response_appearance(OPEN_RESPONSE_ID, adw::ResponseAppearance::Suggested);
                dialog.set_default_response(Some(OPEN_RESPONSE_ID));

                dialog.add_response(NO_RESPONSE_ID, &gettext("No, Thanks"));

                dialog.connect_response(
                    Some(OPEN_RESPONSE_ID),
                    clone!(@weak self as obj => move |_, id| {
                        debug_assert_eq!(id, OPEN_RESPONSE_ID);

                        gtk::UriLauncher::new("https://github.com/SeaDve/Mousai/issues/new?assignees=&labels=&template=bug_report.md").launch(
                            Some(&obj),
                            gio::Cancellable::NONE,
                            |res| {
                                if let Err(err ) = res {
                                    tracing::error!("Failed to open bug report URI: {}", err);
                                }
                            },
                        );
                    }),
                );
            }
            RecognizeErrorKind::NoMatches => {
                const TRY_AGAIN_RESPONSE_ID: &str = "try-again";
                const NO_RESPONSE_ID: &str = "no";

                dialog.add_response(TRY_AGAIN_RESPONSE_ID, &gettext("Try Again"));
                dialog.set_response_appearance(
                    TRY_AGAIN_RESPONSE_ID,
                    adw::ResponseAppearance::Suggested,
                );
                dialog.set_default_response(Some(TRY_AGAIN_RESPONSE_ID));

                dialog.add_response(NO_RESPONSE_ID, &gettext("No, Thanks"));

                dialog.connect_response(
                    Some(TRY_AGAIN_RESPONSE_ID),
                    clone!(@weak self as obj => move |_, id| {
                        debug_assert_eq!(id, TRY_AGAIN_RESPONSE_ID);

                        debug_assert_eq!(obj.imp().recognizer.state(), RecognizerState::Null);
                        WidgetExt::activate_action(&obj, "win.toggle-recognize", None).unwrap();
                    }),
                );
            }
            RecognizeErrorKind::Connection => {
                const OK_RESPONSE_ID: &str = "ok";

                dialog.set_body(&gettext("Please check your internet connection."));

                dialog.add_response(OK_RESPONSE_ID, &gettext("Ok"));
                dialog.set_default_response(Some(OK_RESPONSE_ID));
            }
        }

        dialog.present();
    }

    fn load_window_size(&self) {
        let app = utils::app_instance();
        let settings = app.settings();

        self.set_default_size(settings.window_width(), settings.window_height());

        if settings.window_maximized() {
            self.maximize();
        }
    }

    fn save_window_size(&self) -> Result<()> {
        let app = utils::app_instance();
        let settings = app.settings();

        let (width, height) = self.default_size();

        settings.try_set_window_width(width)?;
        settings.try_set_window_height(height)?;

        settings.try_set_window_maximized(self.is_maximized())?;

        Ok(())
    }

    fn update_toggle_playback_action(&self) {
        self.action_set_enabled("win.toggle-playback", self.imp().player.song().is_some());
    }

    fn update_toggle_search_action(&self) {
        let imp = self.imp();
        let is_main_page_visible =
            imp.stack.visible_child().as_ref() == Some(imp.main_view.upcast_ref());
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
        imp.player
            .connect_error(clone!(@weak self as obj => move |_, _| {
                obj.add_message_toast(&gettext("An error occurred in the player"));
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
    }

    fn update_song_bar_revealer(&self) {
        let imp = self.imp();
        imp.song_bar_revealer.set_reveal_child(
            imp.player.song().is_some() && !imp.main_view.is_selection_mode_active(),
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
