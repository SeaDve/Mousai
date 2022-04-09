use gettextrs::gettext;
use gtk::gio;
use serde::{Deserialize, Serialize};

use super::ExternalLink;

#[derive(Debug, Serialize, Deserialize)]
pub struct SpotifyExternalLink {
    uri: String,
}

impl SpotifyExternalLink {
    pub fn new(uri: &str) -> Self {
        Self {
            uri: uri.to_string(),
        }
    }
}

#[typetag::serde]
impl ExternalLink for SpotifyExternalLink {
    fn activate(&self) {
        gio::AppInfo::launch_default_for_uri(&self.uri, gio::AppLaunchContext::NONE).unwrap();
    }

    fn name(&self) -> String {
        gettext("Spotify")
    }

    fn tooltip_text(&self) -> String {
        gettext("Listen on Spotify")
    }

    fn css_class(&self) -> &'static str {
        "spotify"
    }
}
