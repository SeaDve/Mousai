use gtk::gio;

use crate::config::APP_ID;

#[gsettings_macro::gen_settings(file = "./data/io.github.seadve.Mousai.gschema.xml.in")]
pub struct Settings;

impl Default for Settings {
    fn default() -> Self {
        Self::new(APP_ID)
    }
}
