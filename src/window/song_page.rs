use gettextrs::gettext;
use gtk::{
    gdk,
    glib::{self, clone, closure_local},
    prelude::*,
    subclass::prelude::*,
};

use std::cell::RefCell;

use super::{
    album_cover::AlbumCover,
    external_link_tile::ExternalLinkTile,
    information_row::InformationRow,
    playback_button::{PlaybackButton, PlaybackButtonMode},
    AdaptiveMode, Window,
};
use crate::{
    model::{ExternalLinkWrapper, Song},
    player::{Player, PlayerState},
    utils, Application,
};

const NORMAL_ALBUM_COVER_PIXEL_SIZE: i32 = 180;
const NARROW_ALBUM_COVER_PIXEL_SIZE: i32 = 120;

mod imp {
    use super::*;
    use glib::{subclass::Signal, WeakRef};
    use gtk::CompositeTemplate;
    use once_cell::sync::Lazy;

    #[derive(Debug, Default, CompositeTemplate)]
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
        pub(super) player: RefCell<Option<(WeakRef<Player>, glib::SignalHandlerId)>>, // Player and Player's state notify handler id
        pub(super) bindings: RefCell<Vec<glib::Binding>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for SongPage {
        const NAME: &'static str = "MsaiSongPage";
        type Type = super::SongPage;
        type ParentType = gtk::Widget;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();

            klass.install_action("song-page.toggle-playback", None, |obj, _, _| {
                if let Err(err) = obj.toggle_playback() {
                    log::warn!("Failed to toggle playback: {err:?}");
                    Application::default().show_error(&err.to_string());
                }
            });

            klass.install_action("song-page.remove-song", None, |obj, _, _| {
                if let Some(ref song) = obj.song() {
                    obj.emit_by_name::<()>("song-removed", &[song]);
                }
            });

            klass.install_action("song-page.copy-song", None, |obj, _, _| {
                if let Some(song) = obj.song() {
                    if let Some(display) = gdk::Display::default() {
                        display.clipboard().set_text(&format!(
                            "{} - {}",
                            song.artist(),
                            song.title()
                        ));

                        let toast = adw::Toast::new(&gettext("Copied song to clipboard"));
                        Application::default().add_toast(&toast);
                    }
                } else {
                    log::error!("Failed to copy song: There is no active song in SongPage");
                }
            });
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for SongPage {
        fn signals() -> &'static [Signal] {
            static SIGNALS: Lazy<Vec<Signal>> = Lazy::new(|| {
                vec![Signal::builder(
                    "song-removed",
                    &[Song::static_type().into()],
                    <()>::static_type().into(),
                )
                .build()]
            });

            SIGNALS.as_ref()
        }

        fn properties() -> &'static [glib::ParamSpec] {
            static PROPERTIES: Lazy<Vec<glib::ParamSpec>> = Lazy::new(|| {
                vec![
                    // Song represented by Self
                    glib::ParamSpecObject::builder("song", Song::static_type())
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
                _ => unimplemented!(),
            }
        }

        fn property(&self, obj: &Self::Type, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
            match pspec.name() {
                "song" => obj.song().to_value(),
                _ => unimplemented!(),
            }
        }

        fn constructed(&self, obj: &Self::Type) {
            self.parent_constructed(obj);

            self.external_links_box.connect_child_activated(|_, child| {
                let external_link_tile = child
                    .clone()
                    .downcast::<ExternalLinkTile>()
                    .expect("Expected `ExternalLinkTile` as child");

                utils::spawn(async move {
                    let external_link_wrapper = external_link_tile.external_link();
                    let external_link = external_link_wrapper.inner();
                    let uri = external_link.uri();

                    if let Err(err) = glib::Uri::is_valid(&uri, glib::UriFlags::ENCODED) {
                        log::warn!("Trying to launch an invalid Uri: {err:?}");
                    }

                    if let Err(err) = gtk::show_uri_full_future(
                        external_link_tile
                            .root()
                            .and_then(|root| root.downcast::<gtk::Window>().ok())
                            .as_ref(),
                        &uri,
                        gdk::CURRENT_TIME,
                    )
                    .await
                    {
                        log::warn!("Failed to launch default for uri `{uri}`: {err:?}");
                        Application::default()
                            .show_error(&gettext!("Failed to launch {}", external_link.name()));
                    }
                });
            });
        }

        fn dispose(&self, obj: &Self::Type) {
            while let Some(child) = obj.first_child() {
                child.unparent();
            }
        }
    }

    impl WidgetImpl for SongPage {
        fn realize(&self, obj: &Self::Type) {
            self.parent_realize(obj);

            if let Some(window) = obj.root().and_then(|root| root.downcast::<Window>().ok()) {
                window.connect_adaptive_mode_notify(clone!(@weak obj => move |window| {
                    obj.update_album_cover_pixel_size(window);
                }));

                obj.update_album_cover_pixel_size(&window);
            } else {
                log::error!("Failed to connect to Window.notify::adaptive-mode: SongPage does not have a root");
            }
        }
    }
}

glib::wrapper! {
    pub struct SongPage(ObjectSubclass<imp::SongPage>)
        @extends gtk::Widget;
}

impl SongPage {
    pub fn new() -> Self {
        glib::Object::new(&[]).expect("Failed to create SongPage")
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

        {
            let mut bindings = imp.bindings.borrow_mut();

            for binding in bindings.drain(..) {
                binding.unbind();
            }

            if let Some(ref song) = song {
                bindings.push(
                    song.bind_property("lyrics", &imp.lyrics_label.get(), "label")
                        .flags(glib::BindingFlags::SYNC_CREATE)
                        .build(),
                );
                bindings.push(
                    song.bind_property("lyrics", &imp.lyrics_group.get(), "visible")
                        .transform_to(|_, value| {
                            let lyrics = value.get::<Option<String>>().unwrap();
                            Some(lyrics.is_some().to_value())
                        })
                        .flags(glib::BindingFlags::SYNC_CREATE)
                        .build(),
                );
            }
        }

        // Only crossfade when album art is not loaded to avoid
        // unnecessary crossfading when the album art can be
        // loaded immediately.
        imp.album_cover.set_enable_crossfade(
            song.as_ref()
                .and_then(|song| song.album_art().ok())
                .map_or(true, |album_art| !album_art.is_loaded()),
        );
        imp.album_cover.set_song(song);

        self.update_information();
        self.update_playback_ui();
        self.update_external_links();

        self.notify("song");
    }

    pub fn song(&self) -> Option<Song> {
        self.imp().song.borrow().clone()
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

    fn toggle_playback(&self) -> anyhow::Result<()> {
        if let Some(ref player) = self.player() {
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
                log::error!("Either the player was dropped or not bound in SongPage");
            }
        }
    }

    fn update_external_links(&self) {
        self.imp().external_links_box.bind_model(
            self.song().map(|song| song.external_links()).as_ref(),
            |item| {
                let wrapper: &ExternalLinkWrapper = item.downcast_ref().unwrap();
                ExternalLinkTile::new(wrapper).upcast()
            },
        );
    }

    fn update_information(&self) {
        let song = match self.song() {
            Some(song) => song,
            None => return,
        };

        let imp = self.imp();

        imp.last_heard_row
            .set_data(&song.last_heard().fuzzy_display());
        imp.album_row.set_data(&song.album());

        if let Some(ref release_date) = song.release_date() {
            imp.release_date_row.set_data(release_date);
            imp.release_date_row.set_visible(true);
        } else {
            imp.release_date_row.set_visible(false);
            imp.release_date_row.set_data("");
        }
    }

    fn update_album_cover_pixel_size(&self, window: &Window) {
        self.imp()
            .album_cover
            .set_pixel_size(match window.adaptive_mode() {
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
