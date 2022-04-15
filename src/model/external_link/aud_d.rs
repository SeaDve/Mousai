use gettextrs::gettext;
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
    fn uri(&self) -> String {
        self.uri.to_string()
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
