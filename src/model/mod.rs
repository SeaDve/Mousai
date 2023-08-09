mod external_link;
mod external_links;
mod song;
mod song_filter;
mod song_list;
mod song_sorter;
mod uid;

use fuzzy_matcher::skim::SkimMatcherV2;
use gtk::glib::once_cell::sync::Lazy;

pub use self::{
    external_link::ExternalLink,
    external_links::{ExternalLinkKey, ExternalLinks},
    song::Song,
    song_filter::SongFilter,
    song_list::SongList,
    song_sorter::SongSorter,
    uid::{Uid, UidCodec},
};

static FUZZY_MATCHER: Lazy<SkimMatcherV2> = Lazy::new(SkimMatcherV2::default);
