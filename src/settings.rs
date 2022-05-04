use gtk::{
    gio::{self, prelude::*},
    glib,
};

use crate::{config::APP_ID, core::AudioDeviceClass};

/// Wrapper for Mousai's [`gio::Settings`] with type safety.
///
/// This must be synced with the defined gschema
#[derive(Debug, Clone)]
pub struct Settings(gio::Settings);

impl Settings {
    pub fn new() -> Self {
        Self(gio::Settings::new(APP_ID))
    }

    pub fn create_action(&self, key: &str) -> gio::Action {
        self.0.create_action(key)
    }

    pub fn window_width(&self) -> i32 {
        self.0.int("window-width")
    }

    pub fn set_window_width(&self, value: i32) -> Result<(), glib::BoolError> {
        self.0.set_int("window-width", value)
    }

    pub fn window_height(&self) -> i32 {
        self.0.int("window-height")
    }

    pub fn set_window_height(&self, value: i32) -> Result<(), glib::BoolError> {
        self.0.set_int("window-height", value)
    }

    pub fn is_maximized(&self) -> bool {
        self.0.boolean("is-maximized")
    }

    pub fn set_maximized(&self, value: bool) -> Result<(), glib::BoolError> {
        self.0.set_boolean("is-maximized", value)
    }

    pub fn history(&self) -> glib::GString {
        self.0.string("history")
    }

    pub fn set_history(&self, value: &str) -> Result<(), glib::BoolError> {
        self.0.set_string("history", value)
    }

    pub fn preferred_audio_source(&self) -> AudioDeviceClass {
        match self.0.string("preferred-audio-source").as_str() {
            "microphone" => AudioDeviceClass::Source,
            "desktop-audio" => AudioDeviceClass::Sink,
            invalid => panic!("expected `preferred-audio-source` key found `{invalid}`"),
        }
    }
}

impl Default for Settings {
    fn default() -> Self {
        Self::new()
    }
}
