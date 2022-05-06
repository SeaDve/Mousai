use gtk::gio;

use crate::{config::APP_ID, core::AudioDeviceClass};

#[gsettings_macro::gen_settings(file = "./data/io.github.seadve.Mousai.gschema.xml.in")]
pub struct Settings;

impl Default for Settings {
    fn default() -> Self {
        Self::new(APP_ID)
    }
}

impl From<PreferredAudioSource> for AudioDeviceClass {
    fn from(audio_source: PreferredAudioSource) -> Self {
        match audio_source {
            PreferredAudioSource::Microphone => AudioDeviceClass::Source,
            PreferredAudioSource::DesktopAudio => AudioDeviceClass::Sink,
        }
    }
}
