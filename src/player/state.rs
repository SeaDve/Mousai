use gtk::glib;
use mpris_player::PlaybackStatus;

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, glib::Enum)]
#[enum_type(name = "MsaiPlaybackState")]
pub enum PlayerState {
    #[default]
    Stopped,
    Buffering,
    Paused,
    Playing,
}

impl From<gst_player::PlayerState> for PlayerState {
    fn from(player_state: gst_player::PlayerState) -> Self {
        match player_state {
            gst_player::PlayerState::Stopped => Self::Stopped,
            gst_player::PlayerState::Buffering => Self::Buffering,
            gst_player::PlayerState::Paused => Self::Paused,
            gst_player::PlayerState::Playing => Self::Playing,
            unknown => {
                log::error!("Unknown PlayerState `{unknown}`");
                Self::Stopped
            }
        }
    }
}

impl From<PlayerState> for PlaybackStatus {
    fn from(playback_state: PlayerState) -> Self {
        match playback_state {
            PlayerState::Stopped | PlayerState::Buffering => PlaybackStatus::Stopped,
            PlayerState::Playing => PlaybackStatus::Playing,
            PlayerState::Paused => PlaybackStatus::Paused,
        }
    }
}
