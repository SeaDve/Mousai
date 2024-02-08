use gettextrs::gettext;
use gtk::{
    gdk,
    glib::{self, clone, closure_local, WeakRef},
    prelude::*,
    subclass::prelude::*,
};

use std::cell::{Cell, OnceCell, RefCell};

use super::{
    album_cover::AlbumCover,
    playback_button::{PlaybackButton, PlaybackButtonMode},
    AdaptiveMode,
};
use crate::{
    player::{Player, PlayerState},
    song::Song,
};

const NORMAL_ALBUM_COVER_PIXEL_SIZE: i32 = 180;
const NARROW_ALBUM_COVER_PIXEL_SIZE: i32 = 120;

mod imp {
    use std::marker::PhantomData;

    use glib::subclass::Signal;
    use once_cell::sync::Lazy;

    use super::*;

    #[derive(Default, glib::Properties, gtk::CompositeTemplate)]
    #[properties(wrapper_type = super::SongTile)]
    #[template(resource = "/io/github/seadve/Mousai/ui/song-tile.ui")]
    pub struct SongTile {
        /// Song represented by Self
        #[property(get, set = Self::set_song, explicit_notify)]
        pub(super) song: RefCell<Option<Song>>,
        /// Whether self should be displayed as selected
        #[property(get, set = Self::set_is_selected, explicit_notify)]
        pub(super) is_selected: Cell<bool>,
        /// Whether self is active
        #[property(get = Self::is_active)]
        pub(super) is_active: PhantomData<bool>,
        /// Whether selection mode is active
        #[property(get, set = Self::set_is_selection_mode_active, explicit_notify)]
        pub(super) is_selection_mode_active: Cell<bool>,
        /// Current adaptive mode
        #[property(get, set = Self::set_adaptive_mode, explicit_notify, builder(AdaptiveMode::default()))]
        pub(super) adaptive_mode: Cell<AdaptiveMode>,
        /// Whether to show select button on hover
        #[property(get, set = Self::set_shows_select_button_on_hover, explicit_notify)]
        pub(super) shows_select_button_on_hover: Cell<bool>,

        #[template_child]
        pub(super) overlay: TemplateChild<gtk::Overlay>, // Unused
        #[template_child]
        pub(super) album_cover: TemplateChild<AlbumCover>,
        #[template_child]
        pub(super) new_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub(super) playback_button: TemplateChild<PlaybackButton>,
        #[template_child]
        pub(super) select_button_revealer: TemplateChild<gtk::Revealer>,
        #[template_child]
        pub(super) select_button: TemplateChild<gtk::CheckButton>,

        pub(super) player: RefCell<Option<(WeakRef<Player>, glib::SignalHandlerId)>>, // Player and Player's state notify handler id
        pub(super) select_button_active_notify_handler_id: OnceCell<glib::SignalHandlerId>,
        pub(super) song_binding_group: glib::BindingGroup,
        pub(super) contains_pointer: Cell<bool>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for SongTile {
        const NAME: &'static str = "MsaiSongTile";
        type Type = super::SongTile;
        type ParentType = gtk::Widget;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();

            klass.set_css_name("songtile");

            klass.install_action("song-tile.toggle-playback", None, |obj, _, _| {
                obj.toggle_playback();
            });
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    #[glib::derived_properties]
    impl ObjectImpl for SongTile {
        fn signals() -> &'static [Signal] {
            static SIGNALS: Lazy<Vec<Signal>> =
                Lazy::new(|| vec![Signal::builder("selection-mode-requested").build()]);

            SIGNALS.as_ref()
        }

        fn constructed(&self) {
            self.parent_constructed();

            let obj = self.obj();

            let motion_controller = gtk::EventControllerMotion::new();
            motion_controller.connect_enter(clone!(@weak obj => move |_, _, _| {
                obj.imp().contains_pointer.set(true);
                obj.update_select_button_visibility();
            }));
            motion_controller.connect_leave(clone!(@weak obj => move |_| {
                obj.imp().contains_pointer.set(false);
                obj.update_select_button_visibility();
            }));
            obj.add_controller(motion_controller);

            let gesture_click = gtk::GestureClick::builder()
                .button(gdk::BUTTON_SECONDARY)
                .build();
            gesture_click.connect_released(clone!(@weak obj => move |gesture, _, x, y| {
                gesture.set_state(gtk::EventSequenceState::Claimed);
                if obj.contains(x, y) {
                    obj.emit_by_name::<()>("selection-mode-requested", &[]);
                }
            }));
            obj.add_controller(gesture_click);

            let gesture_long_press = gtk::GestureLongPress::builder()
                .propagation_phase(gtk::PropagationPhase::Capture)
                .build();
            gesture_long_press.connect_pressed(clone!(@weak obj => move |gesture, x, y| {
                gesture.set_state(gtk::EventSequenceState::Claimed);
                if obj.contains(x, y) {
                    obj.emit_by_name::<()>("selection-mode-requested", &[]);
                }
            }));
            obj.add_controller(gesture_long_press);

            self.select_button_active_notify_handler_id
                .set(
                    self.select_button
                        .connect_active_notify(clone!(@weak obj => move |button| {
                            if button.is_active() && !obj.is_selection_mode_active() {
                                obj.emit_by_name::<()>("selection-mode-requested", &[]);
                            }

                            obj.notify_is_active();
                        })),
                )
                .unwrap();

            self.song_binding_group
                .bind("is-newly-heard", &self.new_label.get(), "visible")
                .build();

            obj.update_select_button_tooltip_text();
            obj.update_select_button_visibility();
            obj.update_playback_button_visibility();
            obj.update_album_cover_size();
        }

        fn dispose(&self) {
            self.obj().unbind_player();

            self.dispose_template();
        }
    }

    impl WidgetImpl for SongTile {}

    impl SongTile {
        fn set_song(&self, song: Option<Song>) {
            let obj = self.obj();

            if song == obj.song() {
                return;
            }

            self.song_binding_group.set_source(song.as_ref());

            self.album_cover.set_song(song.as_ref());

            self.song.replace(song);
            obj.update_playback_button_visibility();

            obj.notify_song();
        }

        fn set_is_selected(&self, is_selected: bool) {
            let obj = self.obj();

            if is_selected == obj.is_selected() {
                return;
            }

            self.is_selected.set(is_selected);

            let handler_id = self
                .select_button_active_notify_handler_id
                .get()
                .expect("handler id should be set on constructed");
            self.select_button.block_signal(handler_id);
            self.select_button.set_active(is_selected);
            self.select_button.unblock_signal(handler_id);

            obj.update_select_button_tooltip_text();

            obj.notify_is_selected();
        }

        fn is_active(&self) -> bool {
            self.select_button.is_active()
        }

        fn set_is_selection_mode_active(&self, is_selection_mode_active: bool) {
            let obj = self.obj();

            if is_selection_mode_active == obj.is_selection_mode_active() {
                return;
            }

            self.is_selection_mode_active.set(is_selection_mode_active);
            obj.update_select_button_visibility();
            obj.notify_is_selection_mode_active();
        }

        fn set_adaptive_mode(&self, adaptive_mode: AdaptiveMode) {
            let obj = self.obj();

            if adaptive_mode == obj.adaptive_mode() {
                return;
            }

            self.adaptive_mode.set(adaptive_mode);
            obj.update_album_cover_size();
            obj.notify_adaptive_mode();
        }

        fn set_shows_select_button_on_hover(&self, show_select_button_on_hover: bool) {
            let obj = self.obj();

            if show_select_button_on_hover == obj.shows_select_button_on_hover() {
                return;
            }

            self.shows_select_button_on_hover
                .set(show_select_button_on_hover);
            obj.update_select_button_visibility();
            obj.notify_shows_select_button_on_hover();
        }
    }
}

glib::wrapper! {
    pub struct SongTile(ObjectSubclass<imp::SongTile>)
        @extends gtk::Widget;
}

impl SongTile {
    pub fn new() -> Self {
        glib::Object::new()
    }

    pub fn connect_selection_mode_requested<F>(&self, f: F) -> glib::SignalHandlerId
    where
        F: Fn(&Self) + 'static,
    {
        self.connect_closure(
            "selection-mode-requested",
            true,
            closure_local!(|obj: &Self| {
                f(obj);
            }),
        )
    }

    /// Must only be called once.
    pub fn bind_player(&self, player: &Player) {
        let handler_id = player.connect_state_notify(clone!(@weak self as obj => move |player| {
            obj.update_playback_ui(player);
        }));

        self.imp()
            .player
            .replace(Some((player.downgrade(), handler_id)));

        self.update_playback_ui(player);
    }

    pub fn unbind_player(&self) {
        if let Some((player, handler_id)) = self.imp().player.take() {
            if let Some(player) = player.upgrade() {
                player.disconnect(handler_id);
            }
        }
    }

    fn toggle_playback(&self) {
        if let Some(ref player) = self
            .imp()
            .player
            .borrow()
            .as_ref()
            .and_then(|(player, _)| player.upgrade())
        {
            if let Some(song) = self.song() {
                if player.state() == PlayerState::Playing && player.is_active_song(song.id_ref()) {
                    player.pause();
                } else {
                    player.set_song(Some(song));
                    player.play();
                }
            }
        }
    }

    fn update_playback_ui(&self, player: &Player) {
        if let Some(ref song) = self.song() {
            let imp = self.imp();
            let is_active_song = player.is_active_song(song.id_ref());
            let player_state = player.state();

            if is_active_song && player_state == PlayerState::Buffering {
                imp.playback_button.set_mode(PlaybackButtonMode::Buffering);
            } else if is_active_song && player_state == PlayerState::Playing {
                imp.playback_button.set_mode(PlaybackButtonMode::Pause);
            } else {
                imp.playback_button.set_mode(PlaybackButtonMode::Play);
            }
        }
    }

    fn update_select_button_tooltip_text(&self) {
        let tooltip_text = if self.is_selected() {
            gettext("Unselect")
        } else {
            gettext("Select")
        };

        self.imp()
            .select_button
            .set_tooltip_text(Some(&tooltip_text));
    }

    fn update_select_button_visibility(&self) {
        let imp = self.imp();

        imp.select_button_revealer.set_reveal_child(
            self.is_selection_mode_active()
                || (imp.contains_pointer.get() && self.shows_select_button_on_hover()),
        );
    }

    fn update_playback_button_visibility(&self) {
        self.imp()
            .playback_button
            .set_visible(self.song().and_then(|song| song.playback_link()).is_some());
    }

    fn update_album_cover_size(&self) {
        self.imp()
            .album_cover
            .set_pixel_size(match self.adaptive_mode() {
                AdaptiveMode::Normal => NORMAL_ALBUM_COVER_PIXEL_SIZE,
                AdaptiveMode::Narrow => NARROW_ALBUM_COVER_PIXEL_SIZE,
            });
    }
}

impl Default for SongTile {
    fn default() -> Self {
        Self::new()
    }
}
