mod state;

use gst_player::prelude::*;
use gtk::{
    glib::{self, clone},
    prelude::*,
    subclass::prelude::*,
};
use mpris_player::{Metadata as MprisMetadata, MprisPlayer};
use once_cell::unsync::OnceCell;

use std::{
    cell::{Cell, RefCell},
    sync::Arc,
};

pub use self::state::PlayerState;
use crate::{config::APP_ID, core::ClockTime, model::Song, send, Application};

#[derive(Debug)]
enum Message {
    PositionUpdated(Option<ClockTime>),
    DurationChanged(Option<ClockTime>),
    StateChanged(PlayerState),
    Error(glib::Error),
    Warning(glib::Error),
    EndOfStream,
}

mod imp {
    use super::*;
    use glib::subclass::Signal;
    use once_cell::sync::Lazy;

    #[derive(Debug)]
    pub struct SongPlayer {
        pub(super) song: RefCell<Option<Song>>,
        pub(super) state: Cell<PlayerState>,
        pub(super) position: Cell<Option<ClockTime>>,
        pub(super) duration: Cell<Option<ClockTime>>,

        pub(super) metadata: RefCell<MprisMetadata>,
        pub(super) player: gst_player::Player,
        pub(super) mpris_player: OnceCell<Arc<MprisPlayer>>,
    }

    impl Default for SongPlayer {
        fn default() -> Self {
            Self {
                song: RefCell::default(),
                state: Cell::default(),
                position: Cell::default(),
                duration: Cell::default(),
                metadata: RefCell::new(MprisMetadata::new()),
                player: gst_player::Player::new(None, None),
                mpris_player: OnceCell::default(),
            }
        }
    }

    #[glib::object_subclass]
    impl ObjectSubclass for SongPlayer {
        const NAME: &'static str = "MsaiSongPlayer";
        type Type = super::SongPlayer;
    }

    impl ObjectImpl for SongPlayer {
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
                    glib::ParamSpecObject::new(
                        "song",
                        "Song",
                        "Song represented by Self",
                        Song::static_type(),
                        glib::ParamFlags::READWRITE | glib::ParamFlags::EXPLICIT_NOTIFY,
                    ),
                    glib::ParamSpecEnum::new(
                        "state",
                        "State",
                        "Current state of the player",
                        PlayerState::static_type(),
                        PlayerState::default() as i32,
                        glib::ParamFlags::READABLE,
                    ),
                    glib::ParamSpecBoxed::new(
                        "position",
                        "Position",
                        "Position",
                        ClockTime::static_type(),
                        glib::ParamFlags::READABLE,
                    ),
                    glib::ParamSpecBoxed::new(
                        "duration",
                        "Duration",
                        "Duration",
                        ClockTime::static_type(),
                        glib::ParamFlags::READABLE,
                    ),
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
                        log::warn!("Failed to set song to SongPlayer: {err:?}");
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
    pub struct SongPlayer(ObjectSubclass<imp::SongPlayer>);
}

impl SongPlayer {
    pub fn new() -> Self {
        glib::Object::new(&[]).expect("Failed to create SongPlayer")
    }

    pub fn connect_error<F>(&self, f: F) -> glib::SignalHandlerId
    where
        F: Fn(&Self, &glib::Error) + 'static,
    {
        self.connect_local("error", true, move |values| {
            let obj = values[0].get::<Self>().unwrap();
            let error = values[1].get::<glib::Error>().unwrap();
            f(&obj, &error);
            None
        })
    }

    pub fn connect_song_notify<F>(&self, f: F) -> glib::SignalHandlerId
    where
        F: Fn(&Self) + 'static,
    {
        self.connect_notify_local(Some("song"), move |obj, _| f(obj))
    }

    pub fn set_song(&self, song: Option<Song>) -> anyhow::Result<()> {
        if song == self.song() {
            return Ok(());
        }

        let imp = self.imp();

        imp.player.stop();

        if let Some(ref song) = song {
            let playback_link = song.playback_link().ok_or_else(|| {
                anyhow::anyhow!("Trying to set a song to audio player without playback link")
            })?;
            imp.player.set_uri(Some(&playback_link));
            log::info!("Uri set to {playback_link}");

            // TODO Fill up nones
            imp.metadata.replace(MprisMetadata {
                length: None,
                art_url: song.album_art().ok().map(|album_art| album_art.uri()),
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
        self.mpris_player().set_can_play(song.as_ref().is_some());

        imp.song.replace(song);

        self.notify("song");
        self.set_position(ClockTime::ZERO);
        self.set_duration(ClockTime::ZERO);

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

    pub fn position(&self) -> ClockTime {
        if self.state() == PlayerState::Stopped {
            return ClockTime::ZERO;
        }

        let imp = self.imp();

        if let Some(position) = imp.position.get() {
            return position;
        }

        imp.player.position().unwrap_or_default().into()
    }

    pub fn connect_duration_notify<F>(&self, f: F) -> glib::SignalHandlerId
    where
        F: Fn(&Self) + 'static,
    {
        self.connect_notify_local(Some("duration"), move |obj, _| f(obj))
    }

    pub fn duration(&self) -> ClockTime {
        if self.song().is_none() {
            return ClockTime::ZERO;
        }

        let imp = self.imp();

        if let Some(duration) = imp.duration.get() {
            return duration;
        }

        imp.player.duration().unwrap_or_default().into()
    }

    pub fn is_active_song(&self, song: &Song) -> bool {
        self.song().as_ref() == Some(song)
    }

    pub fn play(&self) {
        self.imp().player.play();
    }

    pub fn pause(&self) {
        self.imp().player.pause();
    }

    pub fn stop(&self) -> anyhow::Result<()> {
        self.set_song(None)
    }

    pub fn seek(&self, position: ClockTime) -> anyhow::Result<()> {
        let position = position.try_into()?;
        self.imp().player.seek(position);
        Ok(())
    }

    fn set_position(&self, position: ClockTime) {
        self.imp().position.set(Some(position));
        self.mpris_player()
            .set_position(position.as_micros() as i64);
        self.notify("position");
    }

    fn set_duration(&self, duration: ClockTime) {
        let imp = self.imp();
        imp.duration.set(Some(duration));
        imp.metadata.borrow_mut().length = Some(duration.as_micros() as i64);
        self.push_mpris_metadata();
        self.notify("duration");
    }

    fn mpris_player(&self) -> &Arc<MprisPlayer> {
        self.imp().mpris_player.get_or_init(|| {
            let mpris_player = MprisPlayer::new(APP_ID.into(), "Mousai".into(), APP_ID.into());

            mpris_player.set_can_raise(true);
            mpris_player.set_can_seek(true);
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
                obj.stop().unwrap_or_else(|err| log::warn!("Failed to stop player: {err:?}"));
            }));

            mpris_player.connect_pause(clone!(@weak self as obj => move || {
                obj.pause();
            }));

            mpris_player.connect_seek(clone!(@weak self as obj => move |offset_micros| {
                let current_position = obj.position();
                let offset = ClockTime::from_micros(offset_micros.abs() as u64);
                let new_position = if offset_micros < 0 {
                    current_position.saturating_sub(offset)
                } else {
                    current_position.saturating_add(offset)
                };
                obj.seek(new_position).unwrap_or_else(|err| log::warn!("Failed to seek to position: {err:?}"));
            }));

            log::info!("Done setting up MPRIS server");

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
                if let Some(position) = position {
                    self.set_position(*position);
                } else {
                    self.set_position(ClockTime::ZERO);
                }
            }
            Message::DurationChanged(duration) => {
                if let Some(duration) = duration {
                    self.set_duration(*duration);
                } else {
                    self.set_duration(ClockTime::ZERO);
                }
            }
            Message::StateChanged(new_state) => {
                let old_state = imp.state.get();
                log::info!("State changed from `{old_state:?}` -> `{new_state:?}`");

                imp.state.set(*new_state);
                self.mpris_player()
                    .set_can_pause(matches!(new_state, PlayerState::Playing));
                self.mpris_player().set_playback_status(self.state().into());

                self.notify("state");
            }
            Message::Error(ref error) => {
                log::error!("Gstreamer: {error:?}");
                self.emit_by_name::<()>("error", &[error]);
            }
            Message::Warning(ref warning) => {
                log::warn!("Gstreamer: {warning:?}");
                self.emit_by_name::<()>("error", &[warning]);
            }
            Message::EndOfStream => {
                log::info!("Got end of stream message");
                self.set_position(ClockTime::ZERO);
            }
        }
    }

    fn setup_player_signals(&self) {
        let imp = self.imp();

        let (sender, receiver) = glib::MainContext::channel(glib::PRIORITY_DEFAULT);

        imp.player
            .connect_position_updated(clone!(@strong sender => move |_, position| {
                send!(sender, Message::PositionUpdated(position.map(|position| position.into())));
            }));

        imp.player
            .connect_duration_changed(clone!(@strong sender => move |_, duration| {
                send!(sender, Message::DurationChanged(duration.map(|duration| duration.into())));
            }));

        imp.player
            .connect_state_changed(clone!(@strong sender => move |_, state| {
                send!(sender, Message::StateChanged(state.into()));
            }));

        imp.player
            .connect_error(clone!(@strong sender => move |_, error| {
                send!(sender, Message::Error(error.clone()));
            }));

        imp.player
            .connect_warning(clone!(@strong sender => move |_, error| {
                send!(sender, Message::Warning(error.clone()));
            }));

        imp.player
            .connect_buffering(clone!(@strong sender => move |_, percent| {
                log::debug!("Buffering ({percent}%)");
            }));

        imp.player
            .connect_end_of_stream(clone!(@strong sender => move |_| {
                send!(sender, Message::EndOfStream);
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

impl Default for SongPlayer {
    fn default() -> Self {
        Self::new()
    }
}
