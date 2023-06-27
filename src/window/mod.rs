mod album_cover;
mod crossfade_paintable;
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
use anyhow::Result;
use gettextrs::gettext;
use gtk::{
    gdk, gio,
    glib::{self, clone},
};
use once_cell::unsync::OnceCell;

use self::{history_view::HistoryView, recognizer_view::RecognizerView, song_bar::SongBar};
use crate::{
    config::PROFILE,
    model::{Song, SongList},
    player::{Player, PlayerState},
    preferences_window::PreferencesWindow,
    recognizer::{RecognizeError, RecognizeErrorKind, Recognizer, RecognizerState, Recordings},
    utils, Application,
};

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, glib::Enum)]
#[enum_type(name = "MsaiAdaptiveMode")]
pub enum AdaptiveMode {
    #[default]
    Normal,
    Narrow,
}

mod imp {
    use super::*;

    #[derive(Default, gtk::CompositeTemplate)]
    #[template(resource = "/io/github/seadve/Mousai/ui/window.ui")]
    pub struct Window {
        #[template_child]
        pub(super) toast_overlay: TemplateChild<adw::ToastOverlay>,
        #[template_child]
        pub(super) toolbar_view: TemplateChild<adw::ToolbarView>,
        #[template_child]
        pub(super) stack: TemplateChild<gtk::Stack>,
        #[template_child]
        pub(super) main_view: TemplateChild<HistoryView>,
        #[template_child]
        pub(super) recognizer_view: TemplateChild<RecognizerView>,
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

            klass.install_action("win.toggle-playback", None, |obj, _, _| {
                let imp = obj.imp();

                if imp.player.state() == PlayerState::Playing {
                    imp.player.pause();
                } else {
                    imp.player.play();
                };
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

            obj.setup_signals();

            obj.load_window_size();
            obj.update_song_bar_visibility();
            obj.update_stack();
            obj.update_toggle_playback_action();
            obj.update_toggle_search_action();
        }
    }

    impl WidgetImpl for Window {}

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
                main_view.push_song_page(song);
                main_view.scroll_to_top();
            }));
        imp.recognizer
            .connect_recording_saved(clone!(@weak self as obj => move |_, cause| {
                obj.present_recording_saved_message(cause);
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
        debug_assert!(
            err.is_permanent(),
            "non permanent errors must be saved instead"
        );

        let dialog = adw::MessageDialog::builder()
            .transient_for(self)
            .modal(true)
            .heading(err.title())
            .build();

        match err.kind() {
            RecognizeErrorKind::OtherPermanent | RecognizeErrorKind::Fingerprint => {
                const NO_RESPONSE_ID: &str = "no";
                const OPEN_RESPONSE_ID: &str = "open";

                dialog.set_body(&gettext(
                    "Please open an issue on GitHub and provide the necessary information",
                ));

                dialog.add_response(NO_RESPONSE_ID, &gettext("No, Thanks"));

                dialog.add_response(OPEN_RESPONSE_ID, &gettext("Open an Issue"));
                dialog
                    .set_response_appearance(OPEN_RESPONSE_ID, adw::ResponseAppearance::Suggested);
                dialog.set_default_response(Some(OPEN_RESPONSE_ID));

                dialog.connect_response(
                    Some(OPEN_RESPONSE_ID),
                    clone!(@weak self as obj => move |_, id| {
                        debug_assert_eq!(id, OPEN_RESPONSE_ID);

                        gtk::UriLauncher::new("https://github.com/SeaDve/Mousai/issues/new?assignees=&labels=&template=bug_report.md").launch(
                            Some(&obj),
                            gio::Cancellable::NONE,
                            |res| {
                                if let Err(err ) = res {
                                    tracing::error!("Failed to open bug report URI: {:?}", err);
                                }
                            },
                        );
                    }),
                );
            }
            RecognizeErrorKind::NoMatches => {
                const NO_RESPONSE_ID: &str = "no";
                const TRY_AGAIN_RESPONSE_ID: &str = "try-again";

                dialog.set_body(&gettext(
                    "Try moving closer to the source or using a different excerpt of the song",
                ));

                dialog.add_response(NO_RESPONSE_ID, &gettext("No, Thanks"));

                dialog.add_response(TRY_AGAIN_RESPONSE_ID, &gettext("Try Again"));
                dialog.set_response_appearance(
                    TRY_AGAIN_RESPONSE_ID,
                    adw::ResponseAppearance::Suggested,
                );
                dialog.set_default_response(Some(TRY_AGAIN_RESPONSE_ID));

                dialog.connect_response(
                    Some(TRY_AGAIN_RESPONSE_ID),
                    clone!(@weak self as obj => move |_, id| {
                        debug_assert_eq!(id, TRY_AGAIN_RESPONSE_ID);

                        debug_assert_eq!(obj.imp().recognizer.state(), RecognizerState::Null);
                        WidgetExt::activate_action(&obj, "win.toggle-recognize", None).unwrap();
                    }),
                );
            }
            RecognizeErrorKind::Connection
            | RecognizeErrorKind::InvalidToken
            | RecognizeErrorKind::TokenLimitReached => {
                unreachable!("recording with non permanent errors must be saved instead")
            }
        }

        dialog.present();
    }

    fn present_recording_saved_message(&self, cause: &RecognizeError) {
        debug_assert!(!cause.is_permanent(), "permanent errors must not be saved");

        let dialog = adw::MessageDialog::builder()
            .transient_for(self)
            .modal(true)
            .heading(gettext("Recording Saved"))
            .build();

        match cause.kind() {
            RecognizeErrorKind::Connection => {
                const OK_RESPONSE_ID: &str = "ok";

                dialog.set_body(&gettext(
                    "The result will be available when you're back online",
                ));

                dialog.add_response(OK_RESPONSE_ID, &gettext("Ok, Got It"));
                dialog.set_default_response(Some(OK_RESPONSE_ID));
            }
            RecognizeErrorKind::TokenLimitReached | RecognizeErrorKind::InvalidToken => {
                const NO_RESPONSE_ID: &str = "no";
                const OPEN_RESPONSE_ID: &str = "open";

                match cause.kind() {
                    RecognizeErrorKind::TokenLimitReached => {
                        dialog.set_body(&gettext(
                            "The result will be available when your token limit is reset. Wait until the limit is reset or open preferences and try setting a different token",
                        ));

                        dialog.add_response(NO_RESPONSE_ID, &gettext("I'll Wait"));
                    }
                    RecognizeErrorKind::InvalidToken => {
                        dialog.set_body(&gettext(
                            "The result will be available when your token is replaced with a valid one. Open preferences and try setting a different token",
                        ));

                        dialog.add_response(NO_RESPONSE_ID, &gettext("Later"));
                    }
                    _ => unreachable!(),
                }

                dialog.add_response(OPEN_RESPONSE_ID, &gettext("Open Preferences"));
                dialog
                    .set_response_appearance(OPEN_RESPONSE_ID, adw::ResponseAppearance::Suggested);
                dialog.set_default_response(Some(OPEN_RESPONSE_ID));

                dialog.connect_response(
                    Some(OPEN_RESPONSE_ID),
                    clone!(@weak self as obj => move |_, id| {
                        debug_assert_eq!(id, OPEN_RESPONSE_ID);

                        let window = PreferencesWindow::new(utils::app_instance().settings());
                        window.set_transient_for(Some(&obj));
                        window.present();

                        let is_focused = window.focus_aud_d_api_token_row();
                        debug_assert!(is_focused, "token row must be focused");
                    }),
                );
            }
            RecognizeErrorKind::NoMatches
            | RecognizeErrorKind::Fingerprint
            | RecognizeErrorKind::OtherPermanent => {
                unreachable!("recordings with permanent errors should not be saved")
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

    fn setup_signals(&self) {
        let imp = self.imp();

        imp.player
            .connect_song_notify(clone!(@weak self as obj => move |_| {
                obj.update_toggle_playback_action();
                obj.update_song_bar_visibility();
            }));
        imp.player
            .connect_error(clone!(@weak self as obj => move |_, _| {
                obj.add_message_toast(&gettext("An error occurred in the player"));
            }));

        imp.song_bar
            .connect_activated(clone!(@weak self as obj => move |_, song| {
                obj.imp().main_view.push_song_page(song);
            }));

        imp.main_view.connect_is_selection_mode_active_notify(
            clone!(@weak self as obj => move |_| {
                obj.update_song_bar_visibility();
            }),
        );

        imp.stack
            .connect_visible_child_notify(clone!(@weak self as obj => move |_| {
                obj.update_toggle_search_action();
            }));
    }

    fn update_song_bar_visibility(&self) {
        let imp = self.imp();
        imp.toolbar_view.set_reveal_bottom_bars(
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
            && imp.main_view.is_on_navigation_main_page()
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
