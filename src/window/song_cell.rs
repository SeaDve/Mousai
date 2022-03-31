use gtk::{
    glib::{self, clone},
    prelude::*,
    subclass::prelude::*,
};

use std::cell::RefCell;

use super::album_art::AlbumArt;
use crate::{core::PlaybackState, model::Song, song_player::SongPlayer};

mod imp {
    use super::*;
    use glib::WeakRef;
    use gtk::CompositeTemplate;
    use once_cell::sync::Lazy;

    #[derive(Debug, Default, CompositeTemplate)]
    #[template(resource = "/io/github/seadve/Mousai/ui/song-cell.ui")]
    pub struct SongCell {
        #[template_child]
        pub album_art: TemplateChild<AlbumArt>,
        #[template_child]
        pub playback_stack: TemplateChild<gtk::Stack>,
        #[template_child]
        pub toggle_playback_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub buffering_spinner: TemplateChild<gtk::Spinner>,

        pub song: RefCell<Option<Song>>,
        pub player: RefCell<Option<WeakRef<SongPlayer>>>,
        pub state_notify_handler_id: RefCell<Option<glib::SignalHandlerId>>,
        pub is_buffering_notify_handler_id: RefCell<Option<glib::SignalHandlerId>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for SongCell {
        const NAME: &'static str = "MsaiSongCell";
        type Type = super::SongCell;
        type ParentType = gtk::Widget;

        fn class_init(klass: &mut Self::Class) {
            Self::bind_template(klass);

            klass.install_action("song-cell.toggle-playback", None, move |obj, _, _| {
                if let Err(err) = obj.toggle_playback() {
                    log::warn!("Failed to toggle playback: {err:?}");
                }
            });
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for SongCell {
        fn properties() -> &'static [glib::ParamSpec] {
            static PROPERTIES: Lazy<Vec<glib::ParamSpec>> = Lazy::new(|| {
                vec![glib::ParamSpecObject::new(
                    "song",
                    "Song",
                    "Song represented by self",
                    Song::static_type(),
                    glib::ParamFlags::READWRITE | glib::ParamFlags::EXPLICIT_NOTIFY,
                )]
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

            obj.update_playback_stack_visibility();
        }

        fn dispose(&self, obj: &Self::Type) {
            while let Some(child) = obj.first_child() {
                child.unparent();
            }
        }
    }

    impl WidgetImpl for SongCell {}
}

glib::wrapper! {
    pub struct SongCell(ObjectSubclass<imp::SongCell>)
        @extends gtk::Widget;
}

impl SongCell {
    pub fn new() -> Self {
        glib::Object::new(&[]).expect("Failed to create SongCell")
    }

    pub fn set_song(&self, song: Option<Song>) {
        if song == self.song() {
            return;
        }

        let imp = self.imp();

        imp.album_art.set_song(song.clone());

        imp.song.replace(song);
        self.update_playback_stack_visibility();

        self.notify("song");
    }

    pub fn song(&self) -> Option<Song> {
        self.imp().song.borrow().clone()
    }

    pub fn bind(&self, player: Option<&SongPlayer>) {
        if let Some(player) = player {
            self.update_playback_ui(player);

            let imp = self.imp();
            imp.state_notify_handler_id
                .replace(Some(player.connect_state_notify(
                    clone!(@weak self as obj, @weak player => move |_| {
                        obj.update_playback_ui(&player);
                    }),
                )));
            imp.is_buffering_notify_handler_id
                .replace(Some(player.connect_is_buffering_notify(
                    clone!(@weak self as obj, @weak player => move |_| {
                        obj.update_playback_ui(&player);
                    }),
                )));
            imp.player.replace(Some(player.downgrade()));
        }
    }

    pub fn unbind(&self) {
        let imp = self.imp();
        if let Some(handler_id) = imp.state_notify_handler_id.take() {
            if let Some(player) = imp.player.take().and_then(|player| player.upgrade()) {
                player.disconnect(handler_id);
            }
        }
    }

    fn toggle_playback(&self) -> anyhow::Result<()> {
        if let Some(ref player) = self
            .imp()
            .player
            .borrow()
            .as_ref()
            .and_then(|player| player.upgrade())
        {
            if let Some(song) = self.song() {
                if player.state() == PlaybackState::Playing && player.is_current_playing(&song) {
                    player.pause()?;
                } else {
                    player.set_song(Some(song))?;
                    player.play()?;
                }
            }
        }

        Ok(())
    }

    fn update_playback_ui(&self, player: &SongPlayer) {
        if let Some(ref song) = self.song() {
            let imp = self.imp();
            let toggle_playback_button = &imp.toggle_playback_button.get();
            let buffering_spinner = &imp.buffering_spinner.get();

            if !player.is_current_playing(song) {
                toggle_playback_button.set_icon_name("media-playback-start-symbolic");
                imp.playback_stack.set_visible_child(toggle_playback_button);
                buffering_spinner.set_spinning(false);
                return;
            }

            if player.is_buffering() {
                imp.playback_stack.set_visible_child(buffering_spinner);
                buffering_spinner.set_spinning(true);
                return;
            }

            imp.playback_stack.set_visible_child(toggle_playback_button);
            buffering_spinner.set_spinning(true);

            match player.state() {
                PlaybackState::Stopped | PlaybackState::Paused | PlaybackState::Loading => {
                    toggle_playback_button.set_icon_name("media-playback-start-symbolic");
                }
                PlaybackState::Playing => {
                    toggle_playback_button.set_icon_name("media-playback-pause-symbolic");
                }
            }
        }
    }

    fn update_playback_stack_visibility(&self) {
        self.imp()
            .playback_stack
            .set_visible(self.song().and_then(|song| song.playback_link()).is_some());
    }
}

impl Default for SongCell {
    fn default() -> Self {
        Self::new()
    }
}
