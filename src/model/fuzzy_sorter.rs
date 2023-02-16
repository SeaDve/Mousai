// SPDX-FileCopyrightText: 2022 John Toohey <john_t@mailo.com>
// SPDX-FileCopyrightText: 2022 Dave Patrick Caberto
// SPDX-License-Identifier: GPL-3.0-or-later

use fuzzy_matcher::FuzzyMatcher;
use gtk::{glib, prelude::*, subclass::prelude::*};

use std::cell::RefCell;

use super::{Song, FUZZY_MATCHER};

mod imp {
    use super::*;

    #[derive(Debug, Default, glib::Properties)]
    #[properties(wrapper_type = super::FuzzySorter)]
    pub struct FuzzySorter {
        /// Search term
        ///
        /// If search is empty, the sorter will sort by last heard.
        /// Otherwise, it will sort by the fuzzy match score.
        #[property(get, set = Self::set_search, explicit_notify)]
        pub(super) search: RefCell<String>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for FuzzySorter {
        const NAME: &'static str = "MsaiFuzzySorter";
        type Type = super::FuzzySorter;
        type ParentType = gtk::Sorter;
    }

    impl ObjectImpl for FuzzySorter {
        crate::derived_properties!();
    }

    impl SorterImpl for FuzzySorter {
        fn compare(&self, item_1: &glib::Object, item_2: &glib::Object) -> gtk::Ordering {
            let song_1 = item_1.downcast_ref::<Song>().unwrap();
            let song_2 = item_2.downcast_ref::<Song>().unwrap();

            let search = self.search.borrow();

            if search.is_empty() {
                song_2.last_heard().cmp(&song_1.last_heard()).into()
            } else {
                let song_1_score = FUZZY_MATCHER.fuzzy_match(&song_1.search_term(), &search);
                let song_2_score = FUZZY_MATCHER.fuzzy_match(&song_2.search_term(), &search);
                song_2_score.cmp(&song_1_score).into()
            }
        }

        fn order(&self) -> gtk::SorterOrder {
            gtk::SorterOrder::Partial
        }
    }

    impl FuzzySorter {
        fn set_search(&self, search: String) {
            let obj = self.obj();

            if search == obj.search() {
                return;
            }

            self.search.replace(search);
            obj.changed(gtk::SorterChange::Different);
            obj.notify_search();
        }
    }
}

glib::wrapper! {
    pub struct FuzzySorter(ObjectSubclass<imp::FuzzySorter>)
        @extends gtk::Sorter;

}

impl FuzzySorter {
    pub fn new() -> Self {
        glib::Object::new()
    }
}

impl Default for FuzzySorter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::{core::DateTime, model::SongId};

    fn new_test_song(last_heard: DateTime, search_term: &str) -> Song {
        let song = Song::builder(&SongId::new(""), search_term, search_term, "").build();
        song.set_last_heard(last_heard);
        song
    }

    #[gtk::test]
    fn compare() {
        let sorter = FuzzySorter::new();

        let old = new_test_song(DateTime::now(), "old");
        let new = new_test_song(DateTime::now(), "new");

        // Match search term, closer (old) song is sorted first (smaller)
        sorter.set_search("old");
        assert_eq!(sorter.compare(&old, &new), gtk::Ordering::Smaller);
        assert_eq!(sorter.compare(&new, &old), gtk::Ordering::Larger);
        assert_eq!(sorter.compare(&new, &new), gtk::Ordering::Equal);
        assert_eq!(sorter.compare(&old, &old), gtk::Ordering::Equal);

        // Match time, newer song is sorted first (smaller)
        sorter.set_search("");
        assert_eq!(sorter.compare(&old, &new), gtk::Ordering::Larger);
        assert_eq!(sorter.compare(&new, &old), gtk::Ordering::Smaller);
        assert_eq!(sorter.compare(&new, &new), gtk::Ordering::Equal);
        assert_eq!(sorter.compare(&old, &old), gtk::Ordering::Equal);

        // Match search term, closer (new) song is sorted first (smaller)
        sorter.set_search("new");
        assert_eq!(sorter.compare(&old, &new), gtk::Ordering::Larger);
        assert_eq!(sorter.compare(&new, &old), gtk::Ordering::Smaller);
        assert_eq!(sorter.compare(&new, &new), gtk::Ordering::Equal);
        assert_eq!(sorter.compare(&old, &old), gtk::Ordering::Equal);
    }
}
