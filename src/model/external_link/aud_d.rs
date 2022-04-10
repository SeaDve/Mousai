use gettextrs::gettext;
use gtk::gio;
use serde::{Deserialize, Serialize};

use super::ExternalLink;

#[derive(Debug, Serialize, Deserialize)]
pub struct AudDExternalLink {
    uri: String,
}

impl AudDExternalLink {
    pub fn new(uri: &str) -> Self {
        Self {
            uri: uri.to_string(),
        }
    }
}

#[typetag::serde]
impl ExternalLink for AudDExternalLink {
    fn activate(&self) {
        gio::AppInfo::launch_default_for_uri(&self.uri, gio::AppLaunchContext::NONE).unwrap();
    }

    fn name(&self) -> String {
        gettext("AudD")
    }

    fn tooltip_text(&self) -> String {
        gettext("Browse on AudD")
    }

    fn icon_name(&self) -> &'static str {
        "microphone-symbolic"
    }

    fn css_class(&self) -> &'static str {
        "audd"
    }
}
