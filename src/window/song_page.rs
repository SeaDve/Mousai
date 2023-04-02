use gettextrs::gettext;
use gtk::{
    glib::{self, clone, closure_local},
    prelude::*,
    subclass::prelude::*,
};

use std::cell::{Cell, RefCell};

use super::{
    album_cover::AlbumCover,
    external_link_tile::ExternalLinkTile,
    information_row::InformationRow,
    playback_button::{PlaybackButton, PlaybackButtonMode},
    AdaptiveMode,
};
use crate::{
    debug_unreachable_or_log,
    model::Song,
    player::{Player, PlayerState},
    utils,
};

const NORMAL_ALBUM_COVER_PIXEL_SIZE: i32 = 180;
const NARROW_ALBUM_COVER_PIXEL_SIZE: i32 = 120;

mod imp {
    use super::*;
    use glib::{subclass::Signal, WeakRef};
    use once_cell::sync::Lazy;

    #[derive(Debug, Default, gtk::CompositeTemplate)]
    #[template(resource = "/io/github/seadve/Mousai/ui/song-page.ui")]
    pub struct SongPage {
        #[template_child]
        pub(super) album_cover: TemplateChild<AlbumCover>,
        #[template_child]
        pub(super) playback_button: TemplateChild<PlaybackButton>,
        #[template_child]
        pub(super) last_heard_row: TemplateChild<InformationRow>,
        #[template_child]
        pub(super) album_row: TemplateChild<InformationRow>,
        #[template_child]
        pub(super) release_date_row: TemplateChild<InformationRow>,
        #[template_child]
        pub(super) external_links_box: TemplateChild<gtk::FlowBox>,
        #[template_child]
        pub(super) lyrics_group: TemplateChild<adw::PreferencesGroup>,
        #[template_child]
        pub(super) lyrics_label: TemplateChild<gtk::Label>,

        pub(super) song: RefCell<Option<Song>>,
        pub(super) adaptive_mode: Cell<AdaptiveMode>,

        pub(super) player: RefCell<Option<(WeakRef<Player>, glib::SignalHandlerId)>>, // Player and Player's state notify handler id
        pub(super) song_binding_group: glib::BindingGroup,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for SongPage {
        const NAME: &'static str = "MsaiSongPage";
        type Type = super::SongPage;
        type ParentType = gtk::Widget;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();

            klass.install_action("song-page.toggle-playback", None, |obj, _, _| {
                obj.toggle_playback();
            });

            klass.install_action("song-page.remove-song", None, |obj, _, _| {
                if let Some(ref song) = obj.song() {
                    obj.emit_by_name::<()>("song-removed", &[song]);
                }
            });

            klass.install_action("song-page.copy-song", None, |obj, _, _| {
                if let Some(song) = obj.song() {
                    obj.display().clipboard().set_text(&song.copy_term());

                    let toast = adw::Toast::new(&gettext("Copied to clipboard"));
                    utils::app_instance().add_toast(toast);
                } else {
                    debug_unreachable_or_log!(
                        "failed to copy song: There is no active song in SongPage"
                    );
                }
            });
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for SongPage {
        fn properties() -> &'static [glib::ParamSpec] {
            static PROPERTIES: Lazy<Vec<glib::ParamSpec>> = Lazy::new(|| {
                vec![
                    // Song represented by Self
                    glib::ParamSpecObject::builder::<Song>("song")
                        .explicit_notify()
                        .build(),
                    // Current adapative mode
                    glib::ParamSpecEnum::builder::<AdaptiveMode>("adaptive-mode")
                        .explicit_notify()
                        .build(),
                ]
            });

            PROPERTIES.as_ref()
        }

        fn set_property(&self, _id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
            let obj = self.obj();

            match pspec.name() {
                "song" => {
                    let song = value.get().unwrap();
                    obj.set_song(song);
                }
                "adaptive-mode" => {
                    let adaptive_mode = value.get().unwrap();
                    obj.set_adaptive_mode(adaptive_mode);
                }
                _ => unimplemented!(),
            }
        }

        fn property(&self, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
            let obj = self.obj();

            match pspec.name() {
                "song" => obj.song().into(),
                "adaptive-mode" => obj.adaptive_mode().into(),
                _ => unimplemented!(),
            }
        }

        fn signals() -> &'static [Signal] {
            static SIGNALS: Lazy<Vec<Signal>> = Lazy::new(|| {
                vec![Signal::builder("song-removed")
                    .param_types([Song::static_type()])
                    .build()]
            });

            SIGNALS.as_ref()
        }

        fn constructed(&self) {
            self.parent_constructed();

            let obj = self.obj();

            self.external_links_box.connect_child_activated(|_, child| {
                let external_link_tile = child
                    .clone()
                    .downcast::<ExternalLinkTile>()
                    .expect("Expected `ExternalLinkTile` as child");

                utils::spawn(async move {
                    external_link_tile.handle_activation().await;
                });
            });

            self.song_binding_group
                .bind("lyrics", &self.lyrics_label.get(), "label")
                .build();
            self.song_binding_group
                .bind("lyrics", &self.lyrics_group.get(), "visible")
                .transform_to(|_, value| {
                    let lyrics = value.get::<Option<String>>().unwrap();
                    Some(lyrics.is_some().into())
                })
                .build();

            obj.update_album_cover_size();
        }

        fn dispose(&self) {
            self.obj().unbind_player();

            self.dispose_template();
        }
    }

    impl WidgetImpl for SongPage {}
}

glib::wrapper! {
    pub struct SongPage(ObjectSubclass<imp::SongPage>)
        @extends gtk::Widget;
}

impl SongPage {
    pub fn new() -> Self {
        glib::Object::new()
    }

    pub fn connect_song_removed<F>(&self, f: F) -> glib::SignalHandlerId
    where
        F: Fn(&Self, &Song) + 'static,
    {
        self.connect_closure(
            "song-removed",
            true,
            closure_local!(|obj: &Self, song: &Song| {
                f(obj, song);
            }),
        )
    }

    pub fn set_song(&self, song: Option<Song>) {
        if song == self.song() {
            return;
        }

        let imp = self.imp();
        imp.song.replace(song.clone());

        imp.song_binding_group.set_source(song.as_ref());

        // Only crossfade when album art is not loaded to avoid
        // unnecessary crossfading when the album art can be
        // loaded immediately.
        imp.album_cover.set_enables_crossfade(
            song.as_ref()
                .and_then(|song| song.album_art().ok())
                .map_or(true, |album_art| !album_art.is_loaded()),
        );
        imp.album_cover.set_song(song);

        self.update_playback_ui();
        self.update_information();

        self.notify("song");
    }

    pub fn song(&self) -> Option<Song> {
        self.imp().song.borrow().clone()
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

    /// Must only be called when no player is bound.
    pub fn bind_player(&self, player: &Player) {
        let handler_id = player.connect_state_notify(clone!(@weak self as obj => move |_| {
            obj.update_playback_ui();
        }));

        self.imp()
            .player
            .replace(Some((player.downgrade(), handler_id)));

        self.update_playback_ui();
    }

    pub fn unbind_player(&self) {
        if let Some((player, handler_id)) = self.imp().player.take() {
            if let Some(player) = player.upgrade() {
                player.disconnect(handler_id);
            }
        }
    }

    /// Returns `None` when player is dropped or not bound.
    fn player(&self) -> Option<Player> {
        self.imp()
            .player
            .borrow()
            .as_ref()
            .and_then(|(player, _)| player.upgrade())
    }

    fn toggle_playback(&self) {
        if let Some(ref player) = self.player() {
            if let Some(song) = self.song() {
                if player.state() == PlayerState::Playing && player.is_active_song(&song) {
                    player.pause();
                } else {
                    player.set_song(Some(song));
                    player.play();
                }
            }
        }
    }

    fn update_playback_ui(&self) {
        let imp = self.imp();
        let song = self.song();

        imp.playback_button.set_visible(
            song.as_ref()
                .and_then(|song| song.playback_link())
                .is_some(),
        );

        if let Some(ref song) = song {
            if let Some(player) = self.player() {
                let is_active_song = player.is_active_song(song);
                let player_state = player.state();

                if is_active_song && player_state == PlayerState::Buffering {
                    imp.playback_button.set_mode(PlaybackButtonMode::Buffering);
                } else if is_active_song && player_state == PlayerState::Playing {
                    imp.playback_button.set_mode(PlaybackButtonMode::Pause);
                } else {
                    imp.playback_button.set_mode(PlaybackButtonMode::Play);
                }
            } else {
                debug_unreachable_or_log!("either the player was dropped or not bound in SongPage");
            }
        }
    }

    fn update_information(&self) {
        let imp = self.imp();

        let song = self.song();
        let song = song.as_ref();

        imp.external_links_box.bind_model(
            song.map(|song| {
                let filter = gtk::CustomFilter::new(|item| {
                    let link = item.downcast_ref().unwrap();
                    ExternalLinkTile::can_handle(link)
                });
                gtk::FilterListModel::new(Some(song.external_links()), Some(filter))
            })
            .as_ref(),
            |item| {
                let link = item.downcast_ref().unwrap();
                ExternalLinkTile::new(link).upcast()
            },
        );

        imp.last_heard_row.set_value(
            song.map(|song| {
                song.last_heard().map_or_else(
                    || gettext("Unknown").into(),
                    |last_heard| last_heard.fuzzy_display(),
                )
            })
            .unwrap_or_default(),
        );
        imp.album_row
            .set_value(song.map(|song| song.album()).unwrap_or_default());
        imp.release_date_row.set_value(
            song.and_then(|song| song.release_date())
                .unwrap_or_default(),
        );
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

impl Default for SongPage {
    fn default() -> Self {
        Self::new()
    }
}
