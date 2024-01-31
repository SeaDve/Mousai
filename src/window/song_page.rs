use adw::{
    prelude::*,
    subclass::{navigation_page::NavigationPageImpl, prelude::*},
};
use gettextrs::gettext;
use gtk::glib::{self, clone, closure_local};

use std::cell::{Cell, RefCell};

use super::{
    album_cover::AlbumCover,
    external_link_tile::ExternalLinkTile,
    information_row::InformationRow,
    playback_button::{PlaybackButton, PlaybackButtonMode},
    AdaptiveMode,
};
use crate::{
    player::{Player, PlayerState},
    song::Song,
    song_list::SongList,
    Application,
};

const NORMAL_ALBUM_COVER_PIXEL_SIZE: i32 = 180;
const NARROW_ALBUM_COVER_PIXEL_SIZE: i32 = 120;

mod imp {
    use super::*;
    use glib::{once_cell::sync::Lazy, subclass::Signal, WeakRef};

    #[derive(Default, glib::Properties, gtk::CompositeTemplate)]
    #[properties(wrapper_type = super::SongPage)]
    #[template(resource = "/io/github/seadve/Mousai/ui/song-page.ui")]
    pub struct SongPage {
        #[property(get, set = Self::set_song, explicit_notify)]
        pub(super) song: RefCell<Option<Song>>,
        #[property(get, set = Self::set_adaptive_mode, explicit_notify, builder(AdaptiveMode::default()))]
        pub(super) adaptive_mode: Cell<AdaptiveMode>,

        #[template_child]
        pub(super) remove_button: TemplateChild<gtk::Button>,
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

        pub(super) player: RefCell<Option<(WeakRef<Player>, glib::SignalHandlerId)>>, // Player and Player's state notify handler id
        pub(super) song_list: RefCell<Option<(WeakRef<SongList>, glib::SignalHandlerId)>>, // SongList and SongList's items changed handler id
        pub(super) song_binding_group: glib::BindingGroup,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for SongPage {
        const NAME: &'static str = "MsaiSongPage";
        type Type = super::SongPage;
        type ParentType = adw::NavigationPage;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();

            klass.install_action("song-page.copy-song", None, |obj, _, _| {
                let song = obj.song().expect("song should be set");
                obj.display().clipboard().set_text(&song.copy_term());
                Application::get()
                    .window()
                    .add_message_toast(&gettext("Copied to clipboard"));
            });
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    #[glib::derived_properties]
    impl ObjectImpl for SongPage {
        fn signals() -> &'static [Signal] {
            static SIGNALS: Lazy<Vec<Signal>> = Lazy::new(|| {
                vec![Signal::builder("song-remove-request")
                    .param_types([Song::static_type()])
                    .build()]
            });

            SIGNALS.as_ref()
        }

        fn constructed(&self) {
            self.parent_constructed();

            let obj = self.obj();

            self.remove_button
                .connect_clicked(clone!(@weak obj => move |_| {
                    if let Some(ref song) = obj.song() {
                        obj.emit_by_name::<()>("song-remove-request", &[song]);
                    }
                }));
            self.playback_button
                .connect_clicked(clone!(@weak obj => move |_| {
                    obj.toggle_playback();
                }));

            self.external_links_box.connect_child_activated(|_, child| {
                let external_link_tile = child.downcast_ref::<ExternalLinkTile>().unwrap();
                external_link_tile.handle_activation();
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

            obj.update_information();
            obj.update_page_title();
            obj.update_album_cover_size();
        }

        fn dispose(&self) {
            let obj = self.obj();

            obj.unbind_player();
            obj.unbind_song_list();
        }
    }

    impl WidgetImpl for SongPage {}
    impl NavigationPageImpl for SongPage {}

    impl SongPage {
        fn set_song(&self, song: Option<Song>) {
            let obj = self.obj();

            if song == obj.song() {
                return;
            }

            self.song_binding_group.set_source(song.as_ref());

            // Only crossfade when album art is not loaded to avoid
            // unnecessary crossfading when the album art can be
            // loaded immediately.
            self.album_cover.set_enables_crossfade(
                song.as_ref()
                    .and_then(|song| song.album_art())
                    .map_or(true, |album_art| !album_art.is_loaded()),
            );
            self.album_cover.set_song(song.as_ref());

            self.song.replace(song);
            obj.update_playback_ui();
            obj.update_remove_button_sensitivity();
            obj.update_information();
            obj.update_page_title();

            obj.notify_song();
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
    }
}

glib::wrapper! {
    pub struct SongPage(ObjectSubclass<imp::SongPage>)
        @extends gtk::Widget, adw::NavigationPage;
}

impl SongPage {
    pub fn new() -> Self {
        glib::Object::new()
    }

    pub fn connect_song_remove_request<F>(&self, f: F) -> glib::SignalHandlerId
    where
        F: Fn(&Self, &Song) + 'static,
    {
        self.connect_closure(
            "song-remove-request",
            true,
            closure_local!(|obj: &Self, song: &Song| {
                f(obj, song);
            }),
        )
    }

    /// Must only be called when no player was already bound.
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

    /// Must only be called when no song list was already bound.
    pub fn bind_song_list(&self, song_list: &SongList) {
        let handler_id = song_list.connect_items_changed(
            clone!(@weak self as obj => move |_, _index, _removed, _added| {
                obj.update_remove_button_sensitivity();
            }),
        );

        self.imp()
            .song_list
            .replace(Some((song_list.downgrade(), handler_id)));

        self.update_remove_button_sensitivity();
    }

    pub fn unbind_song_list(&self) {
        if let Some((song_list, handler_id)) = self.imp().song_list.take() {
            if let Some(song_list) = song_list.upgrade() {
                song_list.disconnect(handler_id);
            }
        }
    }

    fn player(&self) -> Player {
        self.imp()
            .player
            .borrow()
            .as_ref()
            .map(|(player, _)| player)
            .expect("player must be bound")
            .upgrade()
            .expect("player must not be dropped")
    }

    fn toggle_playback(&self) {
        let player = self.player();

        if let Some(song) = self.song() {
            if player.state() == PlayerState::Playing && player.is_active_song(song.id_ref()) {
                player.pause();
            } else {
                player.set_song(Some(song));
                player.play();
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
            let player = self.player();
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

    fn update_remove_button_sensitivity(&self) {
        let imp = self.imp();

        let song_list = self
            .imp()
            .song_list
            .borrow()
            .as_ref()
            .map(|(song_list, _)| song_list)
            .expect("song list must be bound")
            .upgrade()
            .expect("song list must not be dropped");

        imp.remove_button.set_sensitive(
            self.song()
                .map_or(false, |song| song_list.contains(song.id_ref())),
        );
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
                    |last_heard| last_heard.to_local().fuzzy_display(),
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

    fn update_page_title(&self) {
        self.set_title(
            &self
                .song()
                .as_ref()
                .map(|song| song.title())
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
