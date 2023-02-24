// SPDX-FileCopyrightText: 2022 John Toohey <john_t@mailo.com>
// SPDX-FileCopyrightText: 2023 Dave Patrick Caberto
// SPDX-License-Identifier: GPL-3.0-or-later

use fuzzy_matcher::FuzzyMatcher;
use gtk::{glib, prelude::*, subclass::prelude::*};

use std::cell::RefCell;

use super::{Song, FUZZY_MATCHER};

mod imp {
    use super::*;

    #[derive(Debug, Default, glib::Properties)]
    #[properties(wrapper_type = super::SongFilter)]
    pub struct SongFilter {
        /// Search term
        ///
        /// If search is empty, the filter will match all songs.
        #[property(get, set = Self::set_search, explicit_notify)]
        pub(super) search: RefCell<String>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for SongFilter {
        const NAME: &'static str = "MsaiSongFilter";
        type Type = super::SongFilter;
        type ParentType = gtk::Filter;
    }

    impl ObjectImpl for SongFilter {
        crate::derived_properties!();
    }

    impl FilterImpl for SongFilter {
        fn strictness(&self) -> gtk::FilterMatch {
            if self.search.borrow().is_empty() {
                gtk::FilterMatch::All
            } else {
                gtk::FilterMatch::Some
            }
        }

        fn match_(&self, song: &glib::Object) -> bool {
            let song = song.downcast_ref::<Song>().unwrap();

            let search = self.search.borrow();

            if search.is_empty() {
                true
            } else {
                FUZZY_MATCHER
                    .fuzzy_match(&song.search_term(), &search)
                    .is_some()
            }
        }
    }

    impl SongFilter {
        fn set_search(&self, search: &str) {
            let obj = self.obj();
            let old_search = obj.search();
            let search = search.to_lowercase();

            if old_search == search {
                return;
            }

            let change = if search.is_empty() {
                gtk::FilterChange::LessStrict
            } else if search.starts_with(&old_search) {
                gtk::FilterChange::MoreStrict
            } else if old_search.starts_with(&search) {
                gtk::FilterChange::LessStrict
            } else {
                gtk::FilterChange::Different
            };

            self.search.replace(search);
            obj.changed(change);
            obj.notify_search();
        }
    }
}

glib::wrapper! {
    pub struct SongFilter(ObjectSubclass<imp::SongFilter>)
        @extends gtk::Filter;

}

impl SongFilter {
    pub fn new() -> Self {
        glib::Object::new()
    }
}

impl Default for SongFilter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::{cell::RefCell, rc::Rc};

    use crate::model::SongId;

    #[gtk::test]
    fn strictness() {
        let filter = SongFilter::new();
        assert_eq!(filter.strictness(), gtk::FilterMatch::All);

        filter.set_search("foo");
        assert_eq!(filter.strictness(), gtk::FilterMatch::Some);

        filter.set_search("bar");
        assert_eq!(filter.strictness(), gtk::FilterMatch::Some);

        filter.set_search("");
        assert_eq!(filter.strictness(), gtk::FilterMatch::All);
    }

    #[gtk::test]
    fn match_() {
        let filter = SongFilter::new();
        assert!(filter.match_(&Song::builder(&SongId::new_for_test("0"), "foo", "foo", "").build()));
        assert!(filter.match_(&Song::builder(&SongId::new_for_test("1"), "bar", "bar", "").build()));

        filter.set_search("foo");
        assert!(filter.match_(&Song::builder(&SongId::new_for_test("2"), "foo", "foo", "").build()));
        assert!(
            !filter.match_(&Song::builder(&SongId::new_for_test("3"), "bar", "bar", "").build())
        );

        filter.set_search("bar");
        assert!(
            !filter.match_(&Song::builder(&SongId::new_for_test("4"), "foo", "foo", "").build())
        );
        assert!(filter.match_(&Song::builder(&SongId::new_for_test("5"), "bar", "bar", "").build()));

        filter.set_search("");
        assert!(filter.match_(&Song::builder(&SongId::new_for_test("6"), "foo", "foo", "").build()));
        assert!(filter.match_(&Song::builder(&SongId::new_for_test("7"), "bar", "bar", "").build()));
    }

    #[gtk::test]
    fn changed() {
        let filter = SongFilter::new();

        let calls_output = Rc::new(RefCell::new(Vec::new()));

        let calls_output_clone = Rc::clone(&calls_output);
        filter.connect_changed(move |_, change| {
            calls_output_clone.borrow_mut().push(change);
        });
        assert!(filter.search().is_empty());

        filter.set_search("foo");
        assert_eq!(
            calls_output.borrow_mut().pop().unwrap(),
            gtk::FilterChange::MoreStrict
        );

        filter.set_search("f");
        assert_eq!(
            calls_output.borrow_mut().pop().unwrap(),
            gtk::FilterChange::LessStrict
        );

        filter.set_search("bar");
        assert_eq!(
            calls_output.borrow_mut().pop().unwrap(),
            gtk::FilterChange::Different
        );

        filter.set_search("b");
        assert_eq!(
            calls_output.borrow_mut().pop().unwrap(),
            gtk::FilterChange::LessStrict
        );

        filter.set_search("bars");
        assert_eq!(
            calls_output.borrow_mut().pop().unwrap(),
            gtk::FilterChange::MoreStrict
        );

        filter.set_search("");
        assert_eq!(
            calls_output.borrow_mut().pop().unwrap(),
            gtk::FilterChange::LessStrict
        );
    }
}
