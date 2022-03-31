use gtk::{
    glib::{self, clone},
    prelude::*,
    subclass::prelude::*,
};

use std::{cell::RefCell, time::Duration};

use crate::{
    core::{AudioPlayer, ClockTime, PlaybackState},
    model::Song,
};

mod imp {
    use super::*;
    use once_cell::sync::Lazy;

    #[derive(Debug, Default)]
    pub struct SongPlayer {
        pub song: RefCell<Option<Song>>,
        pub audio_player: AudioPlayer,
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
                    obj.notify("state");
                }));

            self.audio_player
                .connect_is_buffering_notify(clone!(@weak obj => move |_| {
                    obj.notify("is-buffering");
                }));

            // Notify position every 500ms
            glib::timeout_add_local(
                Duration::from_millis(500),
                clone!(@weak obj => @default-return Continue(false), move || {
                    if obj.imp().audio_player.state() == PlaybackState::Playing {
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
}

impl Default for SongPlayer {
    fn default() -> Self {
        Self::new()
    }
}
