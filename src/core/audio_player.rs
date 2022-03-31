// Rust rewrite of gstplayer.py from GNOME Music (GPLv2)
// Modified to remove features that will be unused
// See https://gitlab.gnome.org/GNOME/gnome-music/-/blob/master/gnomemusic/gstplayer.py

use gst::prelude::*;
use gtk::{
    glib::{self, clone},
    subclass::prelude::*,
};
use once_cell::unsync::OnceCell;

use std::cell::{Cell, RefCell};

use super::ClockTime;
use crate::RUNTIME;

#[derive(Debug, Clone, Copy, PartialEq, glib::Enum)]
#[enum_type(name = "AudioPlayerPlaybackState")]
pub enum PlaybackState {
    Stopped,
    Loading,
    Paused,
    Playing,
}

impl Default for PlaybackState {
    fn default() -> Self {
        Self::Stopped
    }
}

mod imp {
    use super::*;
    use once_cell::sync::Lazy;

    #[derive(Debug, Default)]
    pub struct AudioPlayer {
        pub player: OnceCell<gst::Pipeline>,

        pub state: Cell<PlaybackState>,
        pub uri: RefCell<String>,
        pub is_buffering: Cell<bool>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for AudioPlayer {
        const NAME: &'static str = "MsaiAudioPlayer";
        type Type = super::AudioPlayer;
    }

    impl ObjectImpl for AudioPlayer {
        fn properties() -> &'static [glib::ParamSpec] {
            static PROPERTIES: Lazy<Vec<glib::ParamSpec>> = Lazy::new(|| {
                vec![
                    glib::ParamSpecEnum::new(
                        "state",
                        "State",
                        "Current state of the player",
                        PlaybackState::static_type(),
                        PlaybackState::default() as i32,
                        glib::ParamFlags::READWRITE | glib::ParamFlags::EXPLICIT_NOTIFY,
                    ),
                    glib::ParamSpecString::new(
                        "uri",
                        "Uri",
                        "Current uri being played in the player",
                        None,
                        glib::ParamFlags::READWRITE | glib::ParamFlags::EXPLICIT_NOTIFY,
                    ),
                    glib::ParamSpecBoolean::new(
                        "is-buffering",
                        "Is Buffering",
                        "Whether this is buffering",
                        false,
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
                "state" => {
                    let state = value.get().unwrap();
                    obj.set_state(state);
                }
                "uri" => {
                    let uri = value.get().unwrap();
                    if let Err(err) = obj.set_uri(uri) {
                        log::warn!("Failed to set player uri to `{uri:?}`: {:?}", err);
                    }
                }
                _ => unimplemented!(),
            }
        }

        fn property(&self, obj: &Self::Type, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
            match pspec.name() {
                "state" => obj.state().to_value(),
                "uri" => obj.uri().to_value(),
                "is-buffering" => obj.is_buffering().to_value(),
                _ => unimplemented!(),
            }
        }

        fn dispose(&self, _: &Self::Type) {
            if let Some(player) = self.player.get() {
                if let Err(err) = player.set_state(gst::State::Null) {
                    log::warn!("Failed to set player state to null: {:?}", err);
                }
            }
        }
    }
}

glib::wrapper! {
    pub struct AudioPlayer(ObjectSubclass<imp::AudioPlayer>);
}

impl AudioPlayer {
    pub fn new() -> Self {
        glib::Object::new(&[]).expect("Failed to create AudioPlayer.")
    }

    pub fn connect_state_notify<F>(&self, f: F) -> glib::SignalHandlerId
    where
        F: Fn(&Self) + 'static,
    {
        self.connect_notify_local(Some("state"), move |obj, _| f(obj))
    }

    pub fn set_state(&self, state: PlaybackState) {
        if let Err(err) = self.try_set_state(state) {
            log::warn!("Failed to set player state to `{state:?}`: {err:?}");
        }
    }

    pub fn try_set_state(&self, state: PlaybackState) -> anyhow::Result<()> {
        let player = self.get_or_try_init_player()?;

        match state {
            PlaybackState::Stopped => {
                player.set_state(gst::State::Null)?;
                log::info!("Player state changed to Stopped");

                self.set_buffering(false);

                // Changing the state to NULL flushes the pipeline.
                // Thus, the change message never arrives.
                self.imp().state.set(state);
                self.notify("state");
            }
            PlaybackState::Loading => {
                player.set_state(gst::State::Ready)?;
            }
            PlaybackState::Paused => {
                player.set_state(gst::State::Paused)?;
            }
            PlaybackState::Playing => {
                player.set_state(gst::State::Playing)?;
            }
        }

        Ok(())
    }

    pub fn state(&self) -> PlaybackState {
        self.imp().state.get()
    }

    pub fn set_uri(&self, uri: &str) -> anyhow::Result<()> {
        log::debug!("Setting uri to `{uri}`");

        let player = self.get_or_try_init_player()?;

        if self.state() == PlaybackState::Stopped {
            player.set_property("uri", uri);
        } else {
            self.set_state(PlaybackState::Stopped);
            player.set_property("uri", uri);
            self.set_state(PlaybackState::Playing);
        }

        self.imp().uri.replace(uri.to_owned());
        self.notify("uri");

        Ok(())
    }

    pub fn uri(&self) -> String {
        self.imp().uri.borrow().clone()
    }

    pub fn connect_is_buffering_notify<F>(&self, f: F) -> glib::SignalHandlerId
    where
        F: Fn(&Self) + 'static,
    {
        self.connect_notify_local(Some("is-buffering"), move |obj, _| f(obj))
    }

    pub fn is_buffering(&self) -> bool {
        self.imp().is_buffering.get()
    }

    pub fn seek(&self, position: ClockTime) -> anyhow::Result<()> {
        let position: gst::ClockTime = position.try_into()?;

        self.get_or_try_init_player()?
            .seek_simple(gst::SeekFlags::FLUSH | gst::SeekFlags::KEY_UNIT, position)?;

        Ok(())
    }

    pub fn query_position(&self) -> anyhow::Result<ClockTime> {
        if self.state() == PlaybackState::Stopped {
            return Ok(ClockTime::ZERO);
        }

        match self
            .get_or_try_init_player()?
            .query_position::<gst::ClockTime>()
        {
            Some(clock_time) => Ok(clock_time.into()),
            None => Err(anyhow::anyhow!("Failed to query position")),
        }
    }

    pub async fn duration(&self) -> anyhow::Result<ClockTime> {
        let uri = self.uri();

        let discover_info = RUNTIME
            .spawn_blocking(move || {
                let timeout = gst::ClockTime::from_seconds(10);
                let discoverer = gst_pbutils::Discoverer::new(timeout).unwrap();
                discoverer.discover_uri(&uri)
            })
            .await??;

        Ok(discover_info
            .duration()
            .map_or(ClockTime::ZERO, |ct| ct.into()))
    }

    fn set_buffering(&self, is_buffering: bool) {
        let imp = self.imp();

        if imp.is_buffering.get() == is_buffering {
            return;
        }

        imp.is_buffering.set(is_buffering);
        self.notify("is-buffering");
    }

    fn get_or_try_init_player(&self) -> anyhow::Result<&gst::Pipeline> {
        self.imp().player.get_or_try_init(|| {
            let player = gst::ElementFactory::make("playbin3", None)?
                .downcast::<gst::Pipeline>()
                .unwrap();

            let bus = player.bus().unwrap();
            bus.add_watch_local(
                clone!(@weak self as obj => @default-return Continue(false), move |_, message| {
                    obj.handle_bus_message(message)
                }),
            )
            .unwrap();

            log::info!("Initialized AudioPlayer's player");

            Ok(player)
        })
    }

    fn handle_clock_lost(&self) -> anyhow::Result<()> {
        self.try_set_state(PlaybackState::Paused)?;
        self.try_set_state(PlaybackState::Playing)?;
        Ok(())
    }

    fn handle_bus_message(&self, message: &gst::Message) -> Continue {
        use gst::MessageView;

        match message.view() {
            MessageView::Buffering(ref message) => {
                let (mode, avg_in, avg_out, buffering_left) = message.buffering_stats();
                log::debug!(
                    "AudioPlayer is buffering; percent `{}`; mode `{mode:?}`; avg_in `{avg_in}`; avg_out `{avg_out}`; buffering_left `{buffering_left}`",
                    message.percent(),
                );

                if message.percent() < 100 {
                    self.set_buffering(true);
                    self.set_state(PlaybackState::Paused);
                } else {
                    self.set_buffering(false);
                    self.set_state(PlaybackState::Playing);
                }
            }
            MessageView::ClockLost(_) => {
                log::info!("Got ClockLost message");

                if let Err(err) = self.handle_clock_lost() {
                    log::warn!("Failed to handle clock lost: {err:?}");
                }
            }
            MessageView::Error(ref message) => {
                let error = message.error();
                let debug = message.debug();

                log::warn!(
                    "Error from element `{}`: {:?}",
                    message.src().unwrap().name(),
                    error
                );

                if let Some(debug) = debug {
                    log::warn!("Debug info: {}", debug);
                }

                log::warn!("Error while playing audio with uri `{}`", self.uri());

                self.set_state(PlaybackState::Stopped);
            }
            MessageView::Eos(_) => {
                self.set_state(PlaybackState::Stopped);
            }
            MessageView::StateChanged(ref message) => {
                if let Some(player) = self.imp().player.get() {
                    if message.src().as_ref() != Some(player.upcast_ref::<gst::Object>()) {
                        return Continue(true);
                    }

                    let old_state = message.old();
                    let new_state = message.current();

                    log::info!(
                        "Player state changed: `{:?}` -> `{:?}`",
                        old_state,
                        new_state
                    );

                    let state = match new_state {
                        gst::State::Null => PlaybackState::Stopped,
                        gst::State::Ready => PlaybackState::Loading,
                        gst::State::Paused => PlaybackState::Paused,
                        gst::State::Playing => PlaybackState::Playing,
                        _ => return Continue(true),
                    };

                    self.imp().state.set(state);
                    self.notify("state");
                } else {
                    log::warn!("Got on StateChanged message even player is not initialized");
                }
            }
            _ => (),
        }

        Continue(true)
    }
}

impl Default for AudioPlayer {
    fn default() -> Self {
        Self::new()
    }
}
