use gettextrs::gettext;
use gtk::gio;
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
    fn activate(&self) {
        let encoded = percent_encoding::utf8_percent_encode(
            &self.search_term,
            percent_encoding::NON_ALPHANUMERIC,
        )
        .to_string();

        gio::AppInfo::launch_default_for_uri(
            &format!("https://www.youtube.com/results?search_query={encoded}"),
            gio::AppLaunchContext::NONE,
        )
        .unwrap();
    }

    fn name(&self) -> String {
        gettext("YouTube")
    }

    fn tooltip_text(&self) -> String {
        gettext("Search on YouTube")
    }

    fn css_class(&self) -> &'static str {
        "youtube"
    }
}
