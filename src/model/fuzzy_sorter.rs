// SPDX-FileCopyrightText: 2022 John Toohey <john_t@mailo.com>
// SPDX-FileCopyrightText: 2022 Dave Patrick Caberto
// SPDX-License-Identifier: GPL-3.0-or-later

use fuzzy_matcher::FuzzyMatcher;
use gtk::{glib, prelude::*, subclass::prelude::*};

use std::cell::RefCell;

use super::{Song, FUZZY_MATCHER};

mod imp {
    use super::*;
    use once_cell::sync::Lazy;

    #[derive(Debug, Default)]
    pub struct FuzzySorter {
        pub(super) search: RefCell<String>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for FuzzySorter {
        const NAME: &'static str = "MsaiFuzzySorter";
        type Type = super::FuzzySorter;
        type ParentType = gtk::Sorter;
    }

    impl ObjectImpl for FuzzySorter {
        fn properties() -> &'static [glib::ParamSpec] {
            static PROPERTIES: Lazy<Vec<glib::ParamSpec>> = Lazy::new(|| {
                vec![
                    // A search term
                    glib::ParamSpecString::builder("search")
                        .explicit_notify()
                        .build(),
                ]
            });

            PROPERTIES.as_ref()
        }

        fn set_property(&self, _id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
            let obj = self.obj();

            match pspec.name() {
                "search" => {
                    let search = value.get().unwrap();
                    obj.set_search(search);
                }
                _ => unimplemented!(),
            }
        }

        fn property(&self, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
            let obj = self.obj();

            match pspec.name() {
                "search" => obj.search().to_value(),
                _ => unimplemented!(),
            }
        }
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
}

glib::wrapper! {
    pub struct FuzzySorter(ObjectSubclass<imp::FuzzySorter>)
        @extends gtk::Sorter;

}

impl FuzzySorter {
    pub fn new() -> Self {
        glib::Object::new(&[])
    }

    pub fn search(&self) -> String {
        self.imp().search.borrow().clone()
    }

    /// If search is empty, the sorter will sort by last heard.
    /// Otherwise, it will sort by the fuzzy match score.
    pub fn set_search(&self, search: &str) {
        if search == self.search() {
            return;
        }

        self.imp().search.replace(search.to_string());
        self.changed(gtk::SorterChange::Different);
        self.notify("search");
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
        let song = Song::builder(&SongId::default(), search_term, search_term, "").build();
        song.set_last_heard(last_heard);
        song
    }

    #[test]
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
