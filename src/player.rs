use anyhow::{anyhow, Result};
use gst_player::prelude::*;
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

use crate::{config::APP_ID, core::ClockTime, model::Song, send, Application};

#[derive(Debug)]
enum Message {
    PositionUpdated(Option<ClockTime>),
    DurationChanged(Option<ClockTime>),
    StateChanged(PlayerState),
    Error(glib::Error),
    Warning(glib::Error),
    Eos,
}

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

    #[derive(Debug)]
    pub struct Player {
        pub(super) song: RefCell<Option<Song>>,
        pub(super) state: Cell<PlayerState>,
        pub(super) position: Cell<Option<ClockTime>>,
        pub(super) duration: Cell<Option<ClockTime>>,

        pub(super) metadata: RefCell<MprisMetadata>,
        pub(super) gst_player: gst_player::Player,
        pub(super) mpris_player: OnceCell<Arc<MprisPlayer>>,
    }

    impl Default for Player {
        fn default() -> Self {
            Self {
                song: RefCell::default(),
                state: Cell::default(),
                position: Cell::default(),
                duration: Cell::default(),
                metadata: RefCell::new(MprisMetadata::new()),
                gst_player: gst_player::Player::new(None, None),
                mpris_player: OnceCell::default(),
            }
        }
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Player {
        const NAME: &'static str = "MsaiPlayer";
        type Type = super::Player;
    }

    impl ObjectImpl for Player {
        fn signals() -> &'static [Signal] {
            static SIGNALS: Lazy<Vec<Signal>> = Lazy::new(|| {
                vec![Signal::builder(
                    "error",
                    &[glib::Error::static_type().into()],
                    <()>::static_type().into(),
                )
                .build()]
            });

            SIGNALS.as_ref()
        }

        fn properties() -> &'static [glib::ParamSpec] {
            static PROPERTIES: Lazy<Vec<glib::ParamSpec>> = Lazy::new(|| {
                vec![
                    // Song being played by the player
                    glib::ParamSpecObject::builder("song", Song::static_type())
                        .flags(glib::ParamFlags::READWRITE | glib::ParamFlags::EXPLICIT_NOTIFY)
                        .build(),
                    // Current state of the player
                    glib::ParamSpecEnum::builder("state", PlayerState::static_type())
                        .default_value(PlayerState::default() as i32)
                        .flags(glib::ParamFlags::READABLE)
                        .build(),
                    // Current position of the player
                    glib::ParamSpecBoxed::builder("position", ClockTime::static_type())
                        .flags(glib::ParamFlags::READABLE)
                        .build(),
                    // Duration of the song
                    glib::ParamSpecBoxed::builder("duration", ClockTime::static_type())
                        .flags(glib::ParamFlags::READABLE)
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
                    if let Err(err) = obj.set_song(song) {
                        tracing::warn!("Failed to set song to Player: {:?}", err);
                    }
                }
                _ => unimplemented!(),
            }
        }

        fn property(&self, obj: &Self::Type, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
            match pspec.name() {
                "song" => obj.song().to_value(),
                "state" => obj.state().to_value(),
                "position" => obj.position().to_value(),
                "duration" => obj.duration().to_value(),
                _ => unimplemented!(),
            }
        }

        fn constructed(&self, obj: &Self::Type) {
            self.parent_constructed(obj);

            obj.setup_player_signals();
        }
    }
}

glib::wrapper! {
    pub struct Player(ObjectSubclass<imp::Player>);
}

impl Player {
    pub fn new() -> Self {
        glib::Object::new(&[]).expect("Failed to create Player")
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

    pub fn connect_song_notify<F>(&self, f: F) -> glib::SignalHandlerId
    where
        F: Fn(&Self) + 'static,
    {
        self.connect_notify_local(Some("song"), move |obj, _| f(obj))
    }

    pub fn set_song(&self, song: Option<Song>) -> Result<()> {
        if song == self.song() {
            return Ok(());
        }

        let imp = self.imp();

        imp.gst_player.stop();
        self.set_position(None);
        self.set_duration(None);

        if let Some(ref song) = song {
            let playback_link = song.playback_link().ok_or_else(|| {
                anyhow!("Trying to set a song to audio player without playback link")
            })?;
            imp.gst_player.set_uri(Some(&playback_link));
            tracing::debug!(uri = playback_link, "Uri changed");

            // TODO Fill up nones
            imp.metadata.replace(MprisMetadata {
                length: None,
                art_url: song
                    .album_art()
                    .ok()
                    .map(|album_art| album_art.uri().into()),
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
            imp.metadata.replace(MprisMetadata::new());
        }
        self.push_mpris_metadata();
        let mpris_player = self.mpris_player();
        mpris_player.set_can_play(song.as_ref().is_some());
        mpris_player.set_can_seek(song.as_ref().is_some());

        imp.song.replace(song);

        self.notify("song");

        Ok(())
    }

    pub fn song(&self) -> Option<Song> {
        self.imp().song.borrow().clone()
    }

    pub fn connect_state_notify<F>(&self, f: F) -> glib::SignalHandlerId
    where
        F: Fn(&Self) + 'static,
    {
        self.connect_notify_local(Some("state"), move |obj, _| f(obj))
    }

    pub fn state(&self) -> PlayerState {
        self.imp().state.get()
    }

    pub fn connect_position_notify<F>(&self, f: F) -> glib::SignalHandlerId
    where
        F: Fn(&Self) + 'static,
    {
        self.connect_notify_local(Some("position"), move |obj, _| f(obj))
    }

    pub fn position(&self) -> Option<ClockTime> {
        self.imp().position.get()
    }

    pub fn connect_duration_notify<F>(&self, f: F) -> glib::SignalHandlerId
    where
        F: Fn(&Self) + 'static,
    {
        self.connect_notify_local(Some("duration"), move |obj, _| f(obj))
    }

    pub fn duration(&self) -> Option<ClockTime> {
        self.imp().duration.get()
    }

    pub fn is_active_song(&self, song: &Song) -> bool {
        self.song().as_ref() == Some(song)
    }

    pub fn play(&self) {
        self.imp().gst_player.play();
    }

    pub fn pause(&self) {
        self.imp().gst_player.pause();
    }

    pub fn stop(&self) -> Result<()> {
        self.set_song(None)
    }

    pub fn seek(&self, position: ClockTime) {
        if matches!(self.state(), PlayerState::Stopped) {
            self.pause();
        }

        tracing::debug!(?position, "Seeking");

        self.imp().gst_player.seek(position.into());
    }

    fn set_position(&self, position: Option<ClockTime>) {
        self.imp().position.set(position);
        self.mpris_player()
            .set_position(position.unwrap_or_default().as_micros() as i64);
        self.notify("position");
    }

    fn set_duration(&self, duration: Option<ClockTime>) {
        let imp = self.imp();
        imp.duration.set(duration);
        imp.metadata.borrow_mut().length = duration.map(|duration| duration.as_micros() as i64);
        self.push_mpris_metadata();
        self.notify("duration");
    }

    fn mpris_player(&self) -> &Arc<MprisPlayer> {
        self.imp().mpris_player.get_or_init(|| {
            let mpris_player = MprisPlayer::new(APP_ID.into(), "Mousai".into(), APP_ID.into());

            mpris_player.set_can_raise(true);
            mpris_player.set_can_set_fullscreen(false);
            mpris_player.set_can_go_previous(false);
            mpris_player.set_can_go_next(false);

            mpris_player.connect_raise(|| {
                Application::default().activate();
            });

            mpris_player.connect_play_pause(clone!(@weak self as obj => move || {
                if obj.state() == PlayerState::Playing {
                    obj.pause();
                } else {
                    obj.play();
                }
            }));

            mpris_player.connect_play(clone!(@weak self as obj => move || {
                obj.play();
            }));

            mpris_player.connect_stop(clone!(@weak self as obj => move || {
                obj.stop().unwrap_or_else(|err| tracing::warn!("Failed to stop player: {:?}", err));
            }));

            mpris_player.connect_pause(clone!(@weak self as obj => move || {
                obj.pause();
            }));

            mpris_player.connect_seek(clone!(@weak self as obj => move |offset_micros| {
                let current_micros = obj.position().unwrap_or_default().as_micros() as i64;
                let new_position = ClockTime::from_micros(current_micros.saturating_add(offset_micros) as u64);
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

    fn handle_player_message(&self, message: &Message) {
        let imp = self.imp();

        match message {
            Message::PositionUpdated(position) => {
                self.set_position(*position);
            }
            Message::DurationChanged(duration) => {
                self.set_duration(*duration);
            }
            Message::StateChanged(new_state) => {
                let old_state = imp.state.get();
                tracing::debug!("State changed from `{:?}` -> `{:?}`", old_state, new_state);

                imp.state.set(*new_state);
                self.mpris_player()
                    .set_can_pause(matches!(new_state, PlayerState::Playing));
                self.mpris_player().set_playback_status(match self.state() {
                    PlayerState::Stopped | PlayerState::Buffering => MprisPlaybackStatus::Stopped,
                    PlayerState::Playing => MprisPlaybackStatus::Playing,
                    PlayerState::Paused => MprisPlaybackStatus::Paused,
                });

                self.notify("state");
            }
            Message::Error(ref error) => {
                tracing::error!(state = ?self.state(), "Received error message: {:?}", error);
                self.emit_by_name::<()>("error", &[error]);
            }
            Message::Warning(ref warning) => {
                tracing::warn!("Received warning message: {:?}", warning);
            }
            Message::Eos => {
                tracing::debug!("Received end of stream message");
                self.set_position(None);
            }
        }
    }

    fn setup_player_signals(&self) {
        let imp = self.imp();

        let (sender, receiver) = glib::MainContext::sync_channel(glib::PRIORITY_DEFAULT, 5);

        imp.gst_player
            .connect_position_updated(clone!(@strong sender => move |_, position| {
                send!(sender, Message::PositionUpdated(position.map(|position| position.into())));
            }));

        imp.gst_player
            .connect_duration_changed(clone!(@strong sender => move |_, duration| {
                send!(sender, Message::DurationChanged(duration.map(|duration| duration.into())));
            }));

        imp.gst_player
            .connect_state_changed(clone!(@strong sender => move |_, state| {
                send!(sender, Message::StateChanged(match state {
                    gst_player::PlayerState::Stopped => PlayerState::Stopped,
                    gst_player::PlayerState::Buffering => PlayerState::Buffering,
                    gst_player::PlayerState::Paused => PlayerState::Paused,
                    gst_player::PlayerState::Playing => PlayerState::Playing,
                    _ => {
                        tracing::warn!("Received unknown PlayerState `{}`", state);
                        return;
                    }
                }));
            }));

        imp.gst_player
            .connect_error(clone!(@strong sender => move |_, error| {
                send!(sender, Message::Error(error.clone()));
            }));

        imp.gst_player
            .connect_warning(clone!(@strong sender => move |_, error| {
                send!(sender, Message::Warning(error.clone()));
            }));

        imp.gst_player
            .connect_buffering(clone!(@strong sender => move |_, percent| {
                tracing::trace!("Buffering ({}%)", percent);
            }));

        imp.gst_player
            .connect_end_of_stream(clone!(@strong sender => move |_| {
                send!(sender, Message::Eos);
            }));

        receiver.attach(
            None,
            clone!(@weak self as obj => @default-return Continue(false), move |message| {
                obj.handle_player_message(&message);
                Continue(true)
            }),
        );
    }
}

impl Default for Player {
    fn default() -> Self {
        Self::new()
    }
}
