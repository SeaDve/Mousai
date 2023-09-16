use gst::bus::BusWatchGuard;
use gst_play::prelude::*;
use gtk::{
    glib::{self, clone, closure_local},
    prelude::*,
    subclass::prelude::*,
};
use mpris_server::{
    async_trait,
    enumflags2::BitFlags,
    zbus::{self, fdo},
    LocalPlayerInterface, LocalRootInterface, LocalServer, LoopStatus, Metadata, PlaybackRate,
    PlaybackStatus, Property, Signal, Time, TrackId, Volume,
};

use std::cell::{Cell, OnceCell, RefCell};

use crate::{
    config::APP_ID,
    model::{Song, Uid},
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
    use glib::{once_cell::sync::Lazy, subclass::Signal};

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

        pub(super) gst_play: gst_play::Play,
        pub(super) bus_watch_guard: OnceCell<BusWatchGuard>,

        pub(super) mpris_server: OnceCell<LocalServer<super::Player>>,
        pub(super) metadata: RefCell<Metadata>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Player {
        const NAME: &'static str = "MsaiPlayer";
        type Type = super::Player;
    }

    #[glib::derived_properties]
    impl ObjectImpl for Player {
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

            let bus_watch_guard = self.gst_play
                .message_bus()
                .add_watch_local(
                    clone!(@weak obj => @default-return glib::ControlFlow::Continue, move |_, message| {
                        if gst_play::Play::is_play_message(message) {
                            let play_message = gst_play::PlayMessage::parse(message).unwrap();
                            obj.handle_gst_play_message(play_message);
                        } else {
                            tracing::trace!("Received other bus message: {:?}", message.view());
                        }
                        glib::ControlFlow::Continue
                    }),
                )
                .unwrap();
            self.bus_watch_guard.set(bus_watch_guard).unwrap();

            match LocalServer::new(APP_ID, obj.clone()) {
                Ok(server) => {
                    self.mpris_server.set(server).unwrap();

                    utils::spawn(
                        glib::Priority::default(),
                        clone!(@weak obj => async move {
                            if let Err(err) = obj.mpris_server().unwrap().init_and_run().await {
                                tracing::error!("Failed to run MPRIS server: {:?}", err);
                            }
                        }),
                    );
                }
                Err(err) => {
                    tracing::error!("Failed to create MPRIS server: {:?}", err);
                }
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

            // FIXME This does not actually reset the position, especially when
            // a song is already playing and we switch to another song that took
            // some time to load.
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
                let mut metadata = Metadata::builder()
                    .album(song.album())
                    .title(song.title())
                    .artist([song.artist()])
                    .build();
                if let Some(album_art) = song.album_art() {
                    metadata.set_art_url(Some(album_art.download_url()));
                }
                self.metadata.replace(metadata);
            } else {
                self.metadata.replace(Metadata::new());
            }

            self.song.replace(song);
            obj.mpris_properties_changed(
                Property::Metadata | Property::CanPlay | Property::CanPause | Property::CanSeek,
            );
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

    pub fn is_active_song(&self, song_id: &Uid) -> bool {
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
        self.notify_position();
    }

    fn set_duration(&self, duration: gst::ClockTime) {
        let imp = self.imp();
        imp.duration.set(duration);
        imp.metadata
            .borrow_mut()
            .set_length(Some(Time::from_micros(duration.useconds() as i64)));
        self.mpris_properties_changed(Property::Metadata);
        self.notify_duration();
    }

    fn mpris_server(&self) -> Option<&LocalServer<Self>> {
        self.imp().mpris_server.get()
    }

    fn mpris_properties_changed(&self, property: impl Into<BitFlags<Property>>) {
        let property = property.into();
        utils::spawn(
            glib::Priority::default(),
            clone!(@weak self as obj => async move {
                if let Some(server) = obj.mpris_server() {
                    if let Err(err) = server.properties_changed(property).await {
                        tracing::error!("Failed to emit MPRIS properties changed: {:?}", err);
                    }
                }
            }),
        );
    }

    fn mpris_seeked(&self, position: Time) {
        utils::spawn(
            glib::Priority::default(),
            clone!(@weak self as obj => async move {
                if let Some(server) = obj.mpris_server() {
                    if let Err(err) = server.emit(Signal::Seeked { position }).await {
                        tracing::error!("Failed to emit MPRIS seeked: {:?}", err);
                    }
                }
            }),
        );
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

                self.mpris_properties_changed(Property::PlaybackStatus);
                self.notify_state();
            }
            PlayMessage::EndOfStream => {
                tracing::debug!("Received end of stream message");
                self.set_position(gst::ClockTime::ZERO);
            }
            PlayMessage::SeekDone => {
                tracing::debug!("Received seek done message");
                let position = imp.gst_play.position().unwrap_or_default();
                self.set_position(position);
                self.mpris_seeked(Time::from_micros(position.useconds() as i64));
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

#[async_trait(?Send)]
impl LocalRootInterface for Player {
    async fn raise(&self) -> fdo::Result<()> {
        utils::app_instance().activate();
        Ok(())
    }

    async fn quit(&self) -> fdo::Result<()> {
        utils::app_instance().quit();
        Ok(())
    }

    async fn can_quit(&self) -> fdo::Result<bool> {
        Ok(true)
    }

    async fn fullscreen(&self) -> fdo::Result<bool> {
        Ok(false)
    }

    async fn set_fullscreen(&self, _fullscreen: bool) -> zbus::Result<()> {
        Err(zbus::Error::from(fdo::Error::NotSupported(
            "Fullscreen is not supported".into(),
        )))
    }

    async fn can_set_fullscreen(&self) -> fdo::Result<bool> {
        Ok(false)
    }

    async fn can_raise(&self) -> fdo::Result<bool> {
        Ok(true)
    }

    async fn has_track_list(&self) -> fdo::Result<bool> {
        Ok(false)
    }

    async fn identity(&self) -> fdo::Result<String> {
        Ok("Mousai".to_string())
    }

    async fn desktop_entry(&self) -> fdo::Result<String> {
        Ok(APP_ID.into())
    }

    async fn supported_uri_schemes(&self) -> fdo::Result<Vec<String>> {
        Ok(vec![])
    }

    async fn supported_mime_types(&self) -> fdo::Result<Vec<String>> {
        Ok(vec![])
    }
}

#[async_trait(?Send)]
impl LocalPlayerInterface for Player {
    async fn next(&self) -> fdo::Result<()> {
        Err(fdo::Error::NotSupported("Next is not supported".into()))
    }

    async fn previous(&self) -> fdo::Result<()> {
        Err(fdo::Error::NotSupported("Previous is not supported".into()))
    }

    async fn pause(&self) -> fdo::Result<()> {
        self.pause();
        Ok(())
    }

    async fn play_pause(&self) -> fdo::Result<()> {
        if self.state() == PlayerState::Playing {
            self.pause();
        } else {
            self.play();
        }
        Ok(())
    }

    async fn stop(&self) -> fdo::Result<()> {
        self.set_song(Song::NONE);
        Ok(())
    }

    async fn play(&self) -> fdo::Result<()> {
        self.play();
        Ok(())
    }

    async fn seek(&self, offset: Time) -> fdo::Result<()> {
        let offset_abs = gst::ClockTime::from_useconds(offset.as_micros().unsigned_abs());
        let new_position = if offset.is_positive() {
            self.position().saturating_add(offset_abs)
        } else {
            self.position().saturating_sub(offset_abs)
        };
        self.seek(new_position);
        Ok(())
    }

    async fn set_position(&self, _track_id: TrackId, _position: Time) -> fdo::Result<()> {
        Err(fdo::Error::NotSupported(
            "SetPosition is not supported".into(),
        ))
    }

    async fn open_uri(&self, _uri: String) -> fdo::Result<()> {
        Err(fdo::Error::NotSupported("OpenUri is not supported".into()))
    }

    async fn playback_status(&self) -> fdo::Result<PlaybackStatus> {
        Ok(match self.state() {
            PlayerState::Stopped | PlayerState::Buffering => PlaybackStatus::Stopped,
            PlayerState::Playing => PlaybackStatus::Playing,
            PlayerState::Paused => PlaybackStatus::Paused,
        })
    }

    async fn loop_status(&self) -> fdo::Result<LoopStatus> {
        Ok(LoopStatus::None)
    }

    async fn set_loop_status(&self, _loop_status: LoopStatus) -> zbus::Result<()> {
        Err(zbus::Error::from(fdo::Error::NotSupported(
            "SetLoopStatus is not supported".into(),
        )))
    }

    async fn rate(&self) -> fdo::Result<PlaybackRate> {
        Ok(1.0)
    }

    async fn set_rate(&self, _rate: PlaybackRate) -> zbus::Result<()> {
        Err(zbus::Error::from(fdo::Error::NotSupported(
            "SetRate is not supported".into(),
        )))
    }

    async fn shuffle(&self) -> fdo::Result<bool> {
        Ok(false)
    }

    async fn set_shuffle(&self, _shuffle: bool) -> zbus::Result<()> {
        Err(zbus::Error::from(fdo::Error::NotSupported(
            "SetShuffle is not supported".into(),
        )))
    }

    async fn metadata(&self) -> fdo::Result<Metadata> {
        Ok(self.imp().metadata.borrow().clone())
    }

    async fn volume(&self) -> fdo::Result<Volume> {
        Ok(1.0)
    }

    async fn set_volume(&self, _volume: Volume) -> zbus::Result<()> {
        Err(zbus::Error::from(fdo::Error::NotSupported(
            "SetVolume is not supported".into(),
        )))
    }

    async fn position(&self) -> fdo::Result<Time> {
        Ok(Time::from_micros(self.position().useconds() as i64))
    }

    async fn minimum_rate(&self) -> fdo::Result<PlaybackRate> {
        Ok(1.0)
    }

    async fn maximum_rate(&self) -> fdo::Result<PlaybackRate> {
        Ok(1.0)
    }

    async fn can_go_next(&self) -> fdo::Result<bool> {
        Ok(false)
    }

    async fn can_go_previous(&self) -> fdo::Result<bool> {
        Ok(false)
    }

    async fn can_play(&self) -> fdo::Result<bool> {
        Ok(self.song().is_some())
    }

    async fn can_pause(&self) -> fdo::Result<bool> {
        Ok(self.song().is_some())
    }

    async fn can_seek(&self) -> fdo::Result<bool> {
        Ok(self.song().is_some())
    }

    async fn can_control(&self) -> fdo::Result<bool> {
        Ok(true)
    }
}
