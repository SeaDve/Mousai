use gettextrs::gettext;
use serde::{Deserialize, Serialize};

use super::ExternalLink;

#[derive(Debug, Serialize, Deserialize)]
pub struct AppleMusicExternalLink {
    uri: String,
}

impl AppleMusicExternalLink {
    pub fn new(uri: &str) -> Self {
        Self {
            uri: uri.to_string(),
        }
    }
}

#[typetag::serde]
impl ExternalLink for AppleMusicExternalLink {
    fn uri(&self) -> String {
        self.uri.to_string()
    }

    fn name(&self) -> String {
        gettext("Apple Music")
    }

    fn tooltip_text(&self) -> String {
        gettext("Browse on Apple Music")
    }

    fn icon_name(&self) -> &'static str {
        "music-note-symbolic"
    }

    fn css_class(&self) -> &'static str {
        "applemusic"
    }
}
