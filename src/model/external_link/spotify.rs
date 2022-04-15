use gettextrs::gettext;
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
    fn uri(&self) -> String {
        self.uri.to_string()
    }

    fn name(&self) -> String {
        gettext("Spotify")
    }

    fn tooltip_text(&self) -> String {
        gettext("Listen on Spotify")
    }

    fn icon_name(&self) -> &'static str {
        "network-wireless-symbolic"
    }

    fn css_class(&self) -> &'static str {
        "spotify"
    }
}
