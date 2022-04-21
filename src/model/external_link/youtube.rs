use gettextrs::gettext;
use gtk::glib;
use serde::{Deserialize, Serialize};

use super::ExternalLink;

#[derive(Debug, Serialize, Deserialize)]
pub struct YoutubeExternalLink {
    search_term: String,
}

impl YoutubeExternalLink {
    pub fn new(search_term: &str) -> Self {
        Self {
            search_term: search_term.to_string(),
        }
    }
}

#[typetag::serde]
impl ExternalLink for YoutubeExternalLink {
    fn uri(&self) -> String {
        let escaped_search_term = glib::Uri::escape_string(&self.search_term, None, true);
        format!("https://www.youtube.com/results?search_query={escaped_search_term}")
    }

    fn name(&self) -> String {
        gettext("YouTube")
    }

    fn tooltip_text(&self) -> String {
        gettext("Search on YouTube")
    }

    fn icon_name(&self) -> &'static str {
        "media-playback-start-symbolic"
    }

    fn css_class(&self) -> &'static str {
        "youtube"
    }
}
