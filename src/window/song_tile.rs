use gtk::{
    gdk,
    glib::{self, clone, closure_local, WeakRef},
    prelude::*,
    subclass::prelude::*,
};
use once_cell::unsync::OnceCell;

use std::cell::{Cell, RefCell};

use super::{
    album_cover::AlbumCover,
    playback_button::{PlaybackButton, PlaybackButtonMode},
    AdaptiveMode,
};
use crate::{
    core::BindingVec,
    model::Song,
    player::{Player, PlayerState},
    Application,
};

const NORMAL_ALBUM_COVER_PIXEL_SIZE: i32 = 180;
const NARROW_ALBUM_COVER_PIXEL_SIZE: i32 = 120;

mod imp {
    use super::*;
    use glib::subclass::Signal;
    use gtk::CompositeTemplate;
    use once_cell::sync::Lazy;

    #[derive(Debug, Default, CompositeTemplate)]
    #[template(resource = "/io/github/seadve/Mousai/ui/song-tile.ui")]
    pub struct SongTile {
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

        pub(super) song: RefCell<Option<Song>>,
        pub(super) is_selected: Cell<bool>,
        pub(super) adaptive_mode: Cell<AdaptiveMode>,
        pub(super) is_selection_mode: Cell<bool>,

        pub(super) player: OnceCell<WeakRef<Player>>,
        pub(super) select_button_active_notify_handler: OnceCell<glib::SignalHandlerId>,
        pub(super) bindings: BindingVec,
        pub(super) contains_pointer: Cell<bool>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for SongTile {
        const NAME: &'static str = "MsaiSongTile";
        type Type = super::SongTile;
        type ParentType = gtk::Widget;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();

            klass.install_action("song-tile.toggle-playback", None, |obj, _, _| {
                if let Err(err) = obj.toggle_playback() {
                    log::warn!("Failed to toggle playback: {err:?}");
                    Application::default().show_error(&err.to_string());
                }
            });
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for SongTile {
        fn properties() -> &'static [glib::ParamSpec] {
            static PROPERTIES: Lazy<Vec<glib::ParamSpec>> = Lazy::new(|| {
                vec![
                    // Song represented by Self
                    glib::ParamSpecObject::builder("song", Song::static_type())
                        .flags(glib::ParamFlags::READWRITE | glib::ParamFlags::EXPLICIT_NOTIFY)
                        .build(),
                    // If self should be displayed as selected
                    glib::ParamSpecBoolean::builder("is-selected")
                        .flags(glib::ParamFlags::READWRITE | glib::ParamFlags::EXPLICIT_NOTIFY)
                        .build(),
                    // If self should be displayed as selected
                    glib::ParamSpecBoolean::builder("is-active")
                        .flags(glib::ParamFlags::READABLE)
                        .build(),
                    // Current selection mode
                    glib::ParamSpecBoolean::builder("is-selection-mode")
                        .flags(glib::ParamFlags::READWRITE | glib::ParamFlags::EXPLICIT_NOTIFY)
                        .build(),
                    // Current adapative mode
                    glib::ParamSpecEnum::builder("adaptive-mode", AdaptiveMode::static_type())
                        .default_value(AdaptiveMode::default() as i32)
                        .flags(glib::ParamFlags::READWRITE | glib::ParamFlags::EXPLICIT_NOTIFY)
                        .build(),
                ]
            });

            PROPERTIES.as_ref()
        }

        fn set_property(
            &self,
            obj: &Self::Type,
            _id: usize,
            value: &glib::Value,
            pspec: &glib::ParamSpec,
        ) {
            match pspec.name() {
                "song" => {
                    let song = value.get().unwrap();
                    obj.set_song(song);
                }
                "is-selected" => {
                    let is_selected = value.get().unwrap();
                    obj.set_selected(is_selected);
                }
                "is-selection-mode" => {
                    let is_selection_mode = value.get().unwrap();
                    obj.set_selection_mode(is_selection_mode);
                }
                "adaptive-mode" => {
                    let adaptive_mode = value.get().unwrap();
                    obj.set_adaptive_mode(adaptive_mode);
                }
                _ => unimplemented!(),
            }
        }

        fn property(&self, obj: &Self::Type, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
            match pspec.name() {
                "song" => obj.song().to_value(),
                "is-selected" => obj.is_selected().to_value(),
                "is-active" => obj.is_active().to_value(),
                "is-selection-mode" => obj.is_selection_mode().to_value(),
                "adaptive-mode" => obj.adaptive_mode().to_value(),
                _ => unimplemented!(),
            }
        }

        fn signals() -> &'static [Signal] {
            static SIGNALS: Lazy<Vec<Signal>> = Lazy::new(|| {
                vec![
                    Signal::builder("request-selection-mode", &[], <()>::static_type().into())
                        .build(),
                ]
            });

            SIGNALS.as_ref()
        }

        fn constructed(&self, obj: &Self::Type) {
            self.parent_constructed(obj);

            let motion_controller = gtk::EventControllerMotion::new();
            motion_controller.connect_enter(clone!(@weak obj => move |_, _, _| {
                obj.imp().contains_pointer.set(true);
                obj.update_select_button_visibility();
            }));
            motion_controller.connect_leave(clone!(@weak obj => move |_| {
                obj.imp().contains_pointer.set(false);
                obj.update_select_button_visibility();
            }));
            obj.add_controller(&motion_controller);

            let gesture_click = gtk::GestureClick::builder()
                .button(gdk::BUTTON_SECONDARY)
                .build();
            gesture_click.connect_released(clone!(@weak obj => move |gesture, _, x, y| {
                gesture.set_state(gtk::EventSequenceState::Claimed);
                if obj.contains(x, y) {
                    obj.emit_by_name::<()>("request-selection-mode", &[]);
                }
            }));
            obj.add_controller(&gesture_click);

            let gesture_long_press = gtk::GestureLongPress::builder().build();
            gesture_long_press.connect_pressed(clone!(@weak obj => move |gesture, x, y| {
                gesture.set_state(gtk::EventSequenceState::Claimed);
                if obj.contains(x, y) {
                    obj.emit_by_name::<()>("request-selection-mode", &[]);
                }
            }));
            obj.add_controller(&gesture_long_press);

            self.select_button_active_notify_handler
                .set(
                    self.select_button
                        .connect_active_notify(clone!(@weak obj => move |button| {
                            if button.is_active() && !obj.is_selection_mode() {
                                obj.emit_by_name::<()>("request-selection-mode", &[]);
                            }

                            obj.notify("is-active");
                        })),
                )
                .unwrap();

            obj.update_select_button_visibility();
            obj.update_playback_button_visibility();
            obj.update_album_cover_size();
        }

        fn dispose(&self, obj: &Self::Type) {
            while let Some(child) = obj.first_child() {
                child.unparent();
            }
        }
    }

    impl WidgetImpl for SongTile {}
}

glib::wrapper! {
    pub struct SongTile(ObjectSubclass<imp::SongTile>)
        @extends gtk::Widget;
}

impl SongTile {
    pub fn new() -> Self {
        glib::Object::new(&[]).expect("Failed to create SongTile")
    }

    pub fn set_song(&self, song: Option<Song>) {
        if song == self.song() {
            return;
        }

        let imp = self.imp();

        imp.bindings.unbind_all();

        if let Some(ref song) = song {
            imp.bindings.push(
                song.bind_property("is-newly-recognized", &imp.new_label.get(), "visible")
                    .flags(glib::BindingFlags::SYNC_CREATE)
                    .build(),
            );
        }

        imp.album_cover.set_song(song.clone());

        imp.song.replace(song);
        self.update_playback_button_visibility();

        self.notify("song");
    }

    pub fn song(&self) -> Option<Song> {
        self.imp().song.borrow().clone()
    }

    pub fn set_selected(&self, selected: bool) {
        if selected == self.is_selected() {
            return;
        }

        let imp = self.imp();

        imp.is_selected.set(selected);

        let handler_id = imp
            .select_button_active_notify_handler
            .get()
            .expect("Handler id was not set on constructed");
        imp.select_button.block_signal(handler_id);
        imp.select_button.set_active(selected);
        imp.select_button.unblock_signal(handler_id);

        self.notify("is-selected");
    }

    pub fn is_selected(&self) -> bool {
        self.imp().is_selected.get()
    }

    pub fn connect_active_notify<F>(&self, f: F) -> glib::SignalHandlerId
    where
        F: Fn(&Self) + 'static,
    {
        self.connect_notify_local(Some("is-active"), move |obj, _| f(obj))
    }

    pub fn is_active(&self) -> bool {
        self.imp().select_button.is_active()
    }

    pub fn set_selection_mode(&self, selection_mode: bool) {
        if selection_mode == self.is_selection_mode() {
            return;
        }

        self.imp().is_selection_mode.set(selection_mode);
        self.update_select_button_visibility();
        self.notify("is-selection-mode");
    }

    pub fn is_selection_mode(&self) -> bool {
        self.imp().is_selection_mode.get()
    }

    pub fn set_adaptive_mode(&self, adaptive_mode: AdaptiveMode) {
        if adaptive_mode == self.adaptive_mode() {
            return;
        }

        self.imp().adaptive_mode.set(adaptive_mode);
        self.update_album_cover_size();
        self.notify("adaptive-mode");
    }

    pub fn adaptive_mode(&self) -> AdaptiveMode {
        self.imp().adaptive_mode.get()
    }

    pub fn connect_request_selection_mode<F>(&self, f: F) -> glib::SignalHandlerId
    where
        F: Fn(&Self) + 'static,
    {
        self.connect_closure(
            "request-selection-mode",
            true,
            closure_local!(|obj: &Self| {
                f(obj);
            }),
        )
    }

    /// Must only be called once.
    pub fn bind_player(&self, player: &Player) {
        player.connect_state_notify(clone!(@weak self as obj, @weak player => move |_| {
            obj.update_playback_ui(&player);
        }));

        self.imp().player.set(player.downgrade()).unwrap();

        self.update_playback_ui(player);
    }

    fn toggle_playback(&self) -> anyhow::Result<()> {
        if let Some(ref player) = self.imp().player.get().and_then(|player| player.upgrade()) {
            if let Some(song) = self.song() {
                if player.state() == PlayerState::Playing && player.is_active_song(&song) {
                    player.pause();
                } else {
                    player.set_song(Some(song))?;
                    player.play();
                }
            }
        }

        Ok(())
    }

    fn update_playback_ui(&self, player: &Player) {
        if let Some(ref song) = self.song() {
            let imp = self.imp();
            let is_active_song = player.is_active_song(song);
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

    fn update_select_button_visibility(&self) {
        let imp = self.imp();

        imp.select_button_revealer
            .set_reveal_child(self.is_selection_mode() || imp.contains_pointer.get());
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
