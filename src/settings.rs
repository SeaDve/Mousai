use gsettings_macro::gen_settings;
use gtk::gio;

use std::collections::HashMap;

use crate::config::APP_ID;

#[gen_settings(file = "./data/io.github.seadve.Mousai.gschema.xml.in")]
#[gen_settings_define(
    key_name = "memory-list",
    arg_type = "Vec<HashMap<String, String>>",
    ret_type = "Vec<HashMap<String, String>>"
)]
pub struct Settings;

impl Default for Settings {
    fn default() -> Self {
        Self::new(APP_ID)
    }
}
