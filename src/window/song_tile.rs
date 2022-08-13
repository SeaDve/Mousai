use gtk::{
    glib::{self, clone},
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
    use glib::WeakRef;
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

        pub(super) song: RefCell<Option<Song>>,
        pub(super) adaptive_mode: Cell<AdaptiveMode>,

        pub(super) player: OnceCell<WeakRef<Player>>,
        pub(super) bindings: BindingVec,
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
                "adaptive-mode" => obj.adaptive_mode().to_value(),
                _ => unimplemented!(),
            }
        }

        fn constructed(&self, obj: &Self::Type) {
            self.parent_constructed(obj);

            obj.update_playback_button_visibility();
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

    pub fn set_adaptive_mode(&self, adaptive_mode: AdaptiveMode) {
        if adaptive_mode == self.adaptive_mode() {
            return;
        }

        let imp = self.imp();

        imp.album_cover.set_pixel_size(match adaptive_mode {
            AdaptiveMode::Normal => NORMAL_ALBUM_COVER_PIXEL_SIZE,
            AdaptiveMode::Narrow => NARROW_ALBUM_COVER_PIXEL_SIZE,
        });

        imp.adaptive_mode.set(adaptive_mode);
        self.notify("adaptive-mode");
    }

    pub fn adaptive_mode(&self) -> AdaptiveMode {
        self.imp().adaptive_mode.get()
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

    fn update_playback_button_visibility(&self) {
        self.imp()
            .playback_button
            .set_visible(self.song().and_then(|song| song.playback_link()).is_some());
    }
}

impl Default for SongTile {
    fn default() -> Self {
        Self::new()
    }
}
