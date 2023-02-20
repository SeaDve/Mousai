mod external_link;
mod external_links;
mod fuzzy_filter;
mod fuzzy_sorter;
mod song;
mod song_id;
mod song_list;

use fuzzy_matcher::skim::SkimMatcherV2;
use once_cell::sync::Lazy;

pub use self::{
    external_link::ExternalLink,
    external_links::{ExternalLinkKey, ExternalLinks},
    fuzzy_filter::FuzzyFilter,
    fuzzy_sorter::FuzzySorter,
    song::Song,
    song_id::SongId,
    song_list::SongList,
};

static FUZZY_MATCHER: Lazy<SkimMatcherV2> = Lazy::new(SkimMatcherV2::default);
