use gtk::{
    glib::{self, clone},
    prelude::*,
    subclass::prelude::*,
};
use mpris_player::{Metadata as MprisMetadata, MprisPlayer, PlaybackStatus as MprisPlaybackStatus};
use once_cell::unsync::OnceCell;

use std::{cell::RefCell, sync::Arc, time::Duration};

use crate::{
    config::APP_ID,
    core::{AudioPlayer, ClockTime, PlaybackState},
    model::Song,
    spawn, Application,
};

mod imp {
    use super::*;
    use once_cell::sync::Lazy;

    #[derive(Debug, Default)]
    pub struct SongPlayer {
        pub song: RefCell<Option<Song>>,
        pub audio_player: AudioPlayer,
        pub mpris_player: OnceCell<Arc<MprisPlayer>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for SongPlayer {
        const NAME: &'static str = "MsaiSongPlayer";
        type Type = super::SongPlayer;
    }

    impl ObjectImpl for SongPlayer {
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
                        PlaybackState::static_type(),
                        PlaybackState::default() as i32,
                        glib::ParamFlags::READABLE,
                    ),
                    glib::ParamSpecBoolean::new(
                        "is-buffering",
                        "Is Buffering",
                        "Whether this is buffering",
                        false,
                        glib::ParamFlags::READABLE,
                    ),
                    glib::ParamSpecBoxed::new(
                        "position",
                        "Position",
                        "Current seek position",
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
                "is-buffering" => obj.is_buffering().to_value(),
                "position" => obj.position().to_value(),
                _ => unimplemented!(),
            }
        }

        fn constructed(&self, obj: &Self::Type) {
            self.parent_constructed(obj);

            self.audio_player
                .connect_state_notify(clone!(@weak obj => move |_| {
                    obj.update_mpris_playback_status();
                    obj.update_mpris_can_pause();

                    if matches!(obj.imp().audio_player.state(), PlaybackState::Stopped) {
                        obj.update_mpris_position();
                        obj.notify("position");
                    }

                    obj.notify("state");
                }));

            self.audio_player
                .connect_is_buffering_notify(clone!(@weak obj => move |_| {
                    obj.notify("is-buffering");
                }));

            // Notify position every 200ms
            glib::timeout_add_local(
                Duration::from_millis(200),
                clone!(@weak obj => @default-return Continue(false), move || {
                    if obj.imp().audio_player.state() == PlaybackState::Playing {
                        obj.update_mpris_position();
                        obj.notify("position");
                    }

                    Continue(true)
                }),
            );
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

    pub fn set_song(&self, song: Option<Song>) -> anyhow::Result<()> {
        if song == self.song() {
            return Ok(());
        }

        let imp = self.imp();

        imp.audio_player.set_state(PlaybackState::Stopped);

        if let Some(ref song) = song {
            let playback_link = song.playback_link().ok_or_else(|| {
                anyhow::anyhow!("Trying to set a song to audio player without playback link")
            })?;
            imp.audio_player.set_uri(&playback_link)?;
        }

        imp.song.replace(song);
        self.update_mpris_metadata();
        self.update_mpris_can_play();

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

    pub fn state(&self) -> PlaybackState {
        self.imp().audio_player.state()
    }

    pub fn connect_is_buffering_notify<F>(&self, f: F) -> glib::SignalHandlerId
    where
        F: Fn(&Self) + 'static,
    {
        self.connect_notify_local(Some("is-buffering"), move |obj, _| f(obj))
    }

    pub fn is_buffering(&self) -> bool {
        self.imp().audio_player.is_buffering()
    }

    pub fn connect_position_notify<F>(&self, f: F) -> glib::SignalHandlerId
    where
        F: Fn(&Self) + 'static,
    {
        self.connect_notify_local(Some("position"), move |obj, _| f(obj))
    }

    pub fn position(&self) -> ClockTime {
        self.imp()
            .audio_player
            .query_position()
            .unwrap_or_else(|err| {
                log::warn!("Failed to query position. Returning default: {err:?}");
                ClockTime::default()
            })
    }

    pub async fn duration(&self) -> anyhow::Result<ClockTime> {
        self.imp().audio_player.duration().await
    }

    pub fn is_current_playing(&self, song: &Song) -> bool {
        self.song().as_ref() == Some(song)
    }

    pub fn play(&self) -> anyhow::Result<()> {
        self.imp()
            .audio_player
            .try_set_state(PlaybackState::Playing)
    }

    pub fn pause(&self) -> anyhow::Result<()> {
        self.imp().audio_player.try_set_state(PlaybackState::Paused)
    }

    pub fn seek(&self, position: ClockTime) -> anyhow::Result<()> {
        self.imp().audio_player.seek(position)
    }

    fn update_mpris_metadata(&self) {
        let mpris_player = self.mpris_player();

        if let Some(song) = self.song() {
            spawn!(clone!(@weak self as obj => async move {
                // TODO: Fill in the Nones
                let duration = obj
                    .imp()
                    .audio_player
                    .duration()
                    .await
                    .map(|duration| duration.as_micros() as i64)
                    .ok();

                obj.mpris_player().set_metadata(MprisMetadata {
                    length: duration,
                    art_url: song.album_art_link(),
                    album: None,
                    album_artist: None,
                    artist: Some(vec![song.artist()]),
                    composer: None,
                    disc_number: None,
                    genre: None,
                    title: Some(song.title()),
                    track_number: None,
                    url: None,
                });
            }));
        } else {
            mpris_player.set_metadata(MprisMetadata::new());
        }
    }

    fn update_mpris_position(&self) {
        self.mpris_player()
            .set_position(self.position().as_micros() as i64);
    }

    fn update_mpris_can_play(&self) {
        self.mpris_player().set_can_play(self.song().is_some());
    }

    fn update_mpris_can_pause(&self) {
        self.mpris_player()
            .set_can_pause(matches!(self.state(), PlaybackState::Playing));
    }

    fn update_mpris_playback_status(&self) {
        let mpris_player = self.mpris_player();
        mpris_player.set_playback_status(match self.imp().audio_player.state() {
            PlaybackState::Stopped | PlaybackState::Loading => MprisPlaybackStatus::Paused,
            PlaybackState::Playing => MprisPlaybackStatus::Playing,
            PlaybackState::Paused => MprisPlaybackStatus::Paused,
        });
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
                if obj.state() == PlaybackState::Playing {
                    obj.pause().unwrap_or_else(|err| log::warn!("Failed to pause SongPlayer: {err:?}"));
                } else {
                    obj.play().unwrap_or_else(|err| log::warn!("Failed to play SongPlayer: {err:?}"));
                }
            }));

            mpris_player.connect_play(clone!(@weak self as obj => move || {
                obj.play().unwrap_or_else(|err| log::warn!("Failed to play SongPlayer: {err:?}"));
            }));

            mpris_player.connect_stop(clone!(@weak self as obj => move || {
                obj.set_song(None).unwrap_or_else(|err| log::warn!("Failed to stop and clear song: {err:?}"));
            }));

            mpris_player.connect_pause(clone!(@weak self as obj => move || {
                obj.pause().unwrap_or_else(|err| log::warn!("Failed to pause SongPlayer: {err:?}"));
            }));

            mpris_player.connect_seek(clone!(@weak self as obj => move |offset_micros| {
                let offset = ClockTime::from_micros(offset_micros.abs() as u64);
                let current_position = obj.position();

                let new_position = if offset_micros < 0 {
                    current_position - offset
                } else {
                    current_position + offset
                };
                obj.seek(new_position).unwrap_or_else(|err| log::warn!("Failed to seek to position: {err:?}"));
            }));

            log::info!("Done setting up MPRIS server");

            mpris_player
        })
    }
}

impl Default for SongPlayer {
    fn default() -> Self {
        Self::new()
    }
}
