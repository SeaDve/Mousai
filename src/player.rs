use gst_play::prelude::*;
use gtk::{
    glib::{self, clone, closure_local},
    prelude::*,
    subclass::prelude::*,
};
use mpris_player::{Metadata as MprisMetadata, MprisPlayer, PlaybackStatus as MprisPlaybackStatus};
use once_cell::unsync::OnceCell;

use std::{
    cell::{Cell, RefCell},
    sync::Arc,
};

use crate::{
    config::APP_ID,
    model::{Song, SongId},
    utils,
};

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, glib::Enum)]
#[enum_type(name = "MsaiPlayerState")]
pub enum PlayerState {
    #[default]
    Stopped,
    Buffering,
    Paused,
    Playing,
}

mod imp {
    use super::*;
    use glib::subclass::Signal;
    use once_cell::sync::Lazy;

    #[derive(Default, glib::Properties)]
    #[properties(wrapper_type = super::Player)]
    pub struct Player {
        /// Song being played. If the song is None, the player will stop.
        #[property(get, set = Self::set_song, explicit_notify, nullable)]
        pub(super) song: RefCell<Option<Song>>,
        /// Current state of the player
        #[property(get, builder(PlayerState::default()))]
        pub(super) state: Cell<PlayerState>,
        /// Current position of the player
        #[property(get)]
        pub(super) position: Cell<gst::ClockTime>,
        /// Duration of the active song
        #[property(get)]
        pub(super) duration: Cell<gst::ClockTime>,

        pub(super) metadata: RefCell<MprisMetadata>,
        pub(super) gst_play: gst_play::Play,
        pub(super) mpris_player: OnceCell<Arc<MprisPlayer>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Player {
        const NAME: &'static str = "MsaiPlayer";
        type Type = super::Player;
    }

    impl ObjectImpl for Player {
        crate::derived_properties!();

        fn signals() -> &'static [Signal] {
            static SIGNALS: Lazy<Vec<Signal>> = Lazy::new(|| {
                vec![Signal::builder("error")
                    .param_types([glib::Error::static_type()])
                    .build()]
            });

            SIGNALS.as_ref()
        }

        fn constructed(&self) {
            self.parent_constructed();

            let obj = self.obj();

            self.gst_play
                .message_bus()
                .add_watch_local(
                    clone!(@weak obj => @default-return Continue(false), move |_, message| {
                        if gst_play::Play::is_play_message(message) {
                            let play_message = gst_play::PlayMessage::parse(message).unwrap();
                            obj.handle_gst_play_message(play_message);
                        } else {
                            tracing::trace!("Received other bus message: {:?}", message.view());
                        }
                        Continue(true)
                    }),
                )
                .unwrap();
        }

        fn dispose(&self) {
            if let Err(err) = self.gst_play.message_bus().remove_watch() {
                tracing::warn!("Failed to remove message bus watch: {:?}", err);
            }
        }
    }

    impl Player {
        fn set_song(&self, song: Option<Song>) {
            let obj = self.obj();

            if song == obj.song() {
                return;
            }

            self.gst_play.stop();
            obj.set_position(gst::ClockTime::ZERO);
            obj.set_duration(gst::ClockTime::ZERO);

            if let Some(ref song) = song {
                let Some(playback_link) = song.playback_link() else {
                    tracing::warn!("Trying to put a song without playback link on the Player");
                    return;
                };

                self.gst_play.set_uri(Some(&playback_link));
                tracing::debug!(uri = playback_link, "Uri changed");

                // TODO Fill up nones
                self.metadata.replace(MprisMetadata {
                    length: None,
                    art_url: song
                        .album_art()
                        .and_then(|album_art| album_art.ok())
                        .map(|album_art| album_art.uri()),
                    album: Some(song.album()),
                    album_artist: None,
                    artist: Some(vec![song.artist()]),
                    composer: None,
                    disc_number: None,
                    genre: None,
                    title: Some(song.title()),
                    track_number: None,
                    url: None,
                });
            } else {
                self.metadata.replace(MprisMetadata::new());
            }
            obj.push_mpris_metadata();
            let mpris_player = obj.mpris_player();
            mpris_player.set_can_play(song.as_ref().is_some());
            mpris_player.set_can_seek(song.as_ref().is_some());

            self.song.replace(song);

            obj.notify_song();
        }
    }
}

glib::wrapper! {
    pub struct Player(ObjectSubclass<imp::Player>);
}

impl Player {
    pub fn new() -> Self {
        glib::Object::new()
    }

    pub fn connect_error<F>(&self, f: F) -> glib::SignalHandlerId
    where
        F: Fn(&Self, &glib::Error) + 'static,
    {
        self.connect_closure(
            "error",
            true,
            closure_local!(|obj: &Self, error: &glib::Error| {
                f(obj, error);
            }),
        )
    }

    pub fn is_active_song(&self, song_id: &SongId) -> bool {
        self.song().map_or(false, |song| song.id_ref() == song_id)
    }

    pub fn play(&self) {
        self.imp().gst_play.play();
    }

    pub fn pause(&self) {
        self.imp().gst_play.pause();
    }

    pub fn seek(&self, position: gst::ClockTime) {
        if matches!(self.state(), PlayerState::Stopped) {
            self.pause();
        }

        tracing::debug!(?position, "Seeking");

        self.imp().gst_play.seek(position);
    }

    fn set_position(&self, position: gst::ClockTime) {
        self.imp().position.set(position);
        self.mpris_player().set_position(position.mseconds() as i64);
        self.notify_position();
    }

    fn set_duration(&self, duration: gst::ClockTime) {
        let imp = self.imp();
        imp.duration.set(duration);
        imp.metadata.borrow_mut().length = Some(duration.mseconds() as i64);
        self.push_mpris_metadata();
        self.notify_duration();
    }

    fn mpris_player(&self) -> &MprisPlayer {
        self.imp().mpris_player.get_or_init(|| {
            let mpris_player = MprisPlayer::new(APP_ID.into(), "Mousai".into(), APP_ID.into());

            mpris_player.set_can_raise(true);
            mpris_player.set_can_set_fullscreen(false);
            mpris_player.set_can_go_previous(false);
            mpris_player.set_can_go_next(false);

            mpris_player.connect_raise(|| {
                tracing::debug!("Raise via MPRIS");
                utils::app_instance().activate();
            });

            mpris_player.connect_play_pause(clone!(@weak self as obj => move || {
                tracing::debug!("Play/Pause via MPRIS");
                if obj.state() == PlayerState::Playing {
                    obj.pause();
                } else {
                    obj.play();
                }
            }));

            mpris_player.connect_play(clone!(@weak self as obj => move || {
                tracing::debug!("Play via MPRIS");
                obj.play();
            }));

            mpris_player.connect_stop(clone!(@weak self as obj => move || {
                tracing::debug!("Stop via MPRIS");
                obj.set_song(Song::NONE);
            }));

            mpris_player.connect_pause(clone!(@weak self as obj => move || {
                tracing::debug!("Pause via MPRIS");
                obj.pause();
            }));

            mpris_player.connect_seek(clone!(@weak self as obj => move |offset_micros| {
                tracing::debug!(?offset_micros, "Seek via MPRIS");
                let current_micros = obj.position().mseconds() as i64;
                let new_position = gst::ClockTime::from_mseconds(current_micros.saturating_add(offset_micros) as u64);
                obj.seek(new_position);
            }));

            tracing::debug!("Done setting up MPRIS server");

            mpris_player
        })
    }

    fn push_mpris_metadata(&self) {
        let current_metadata = self.imp().metadata.borrow().clone();
        self.mpris_player().set_metadata(current_metadata);
    }

    fn handle_gst_play_message(&self, message: gst_play::PlayMessage) {
        use gst_play::{PlayMessage, PlayState};

        let imp = self.imp();

        match message {
            PlayMessage::PositionUpdated { position } => {
                self.set_position(position.unwrap_or_default());
            }
            PlayMessage::DurationChanged { duration } => {
                self.set_duration(duration.unwrap_or_default());
            }
            PlayMessage::StateChanged { state } => {
                let new_state = match state {
                    PlayState::Stopped => PlayerState::Stopped,
                    PlayState::Buffering => PlayerState::Buffering,
                    PlayState::Paused => PlayerState::Paused,
                    PlayState::Playing => PlayerState::Playing,
                    _ => {
                        tracing::warn!("Received unknown PlayState `{}`", state);
                        return;
                    }
                };

                let old_state = imp.state.get();
                tracing::debug!("State changed from `{:?}` -> `{:?}`", old_state, new_state);

                imp.state.set(new_state);
                self.mpris_player()
                    .set_can_pause(matches!(new_state, PlayerState::Playing));
                self.mpris_player().set_playback_status(match self.state() {
                    PlayerState::Stopped | PlayerState::Buffering => MprisPlaybackStatus::Stopped,
                    PlayerState::Playing => MprisPlaybackStatus::Playing,
                    PlayerState::Paused => MprisPlaybackStatus::Paused,
                });

                self.notify_state();
            }
            PlayMessage::EndOfStream => {
                tracing::debug!("Received end of stream message");
                self.set_position(gst::ClockTime::ZERO);
            }
            PlayMessage::SeekDone => {
                tracing::debug!("Received seek done message");
                self.set_position(imp.gst_play.position().unwrap_or_default());
            }
            PlayMessage::Error { error, details } => {
                tracing::error!(state = ?self.state(), ?details, "Received error message: {:?}", error);
                self.emit_by_name::<()>("error", &[&error]);
            }
            PlayMessage::Warning { error, details } => {
                tracing::warn!(?details, "Received warning message: {:?}", error);
            }
            PlayMessage::Buffering { percent } => {
                tracing::trace!("Buffering ({}%)", percent);
            }
            PlayMessage::MediaInfoUpdated { info } => {
                tracing::trace!(
                    container_format = ?info.container_format(),
                    duration = ?info.duration(),
                    stream_list = ?info
                        .stream_list()
                        .iter()
                        .map(|i| format!("{}: {:?}", i.stream_type(), i.codec()))
                        .collect::<Vec<_>>(),
                    tags = ?info.tags(),
                    title = ?info.title(),
                    is_live = info.is_live(),
                    is_seekable = info.is_seekable(),
                    "Received media info update"
                );
            }
            _ => {
                tracing::trace!(?message, "Received other PlayMessage");
            }
        }
    }
}

impl Default for Player {
    fn default() -> Self {
        Self::new()
    }
}
