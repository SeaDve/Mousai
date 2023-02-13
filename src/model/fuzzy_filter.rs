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
    pub struct FuzzyFilter {
        pub(super) search: RefCell<String>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for FuzzyFilter {
        const NAME: &'static str = "MsaiFuzzyFilter";
        type Type = super::FuzzyFilter;
        type ParentType = gtk::Filter;
    }

    impl ObjectImpl for FuzzyFilter {
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

    impl FilterImpl for FuzzyFilter {
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
}

glib::wrapper! {
    pub struct FuzzyFilter(ObjectSubclass<imp::FuzzyFilter>)
        @extends gtk::Filter;

}

impl FuzzyFilter {
    pub fn new() -> Self {
        glib::Object::new()
    }

    pub fn search(&self) -> String {
        self.imp().search.borrow().clone()
    }

    pub fn set_search(&self, search: &str) {
        let old_search = self.search();
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

        self.imp().search.replace(search);
        self.changed(change);
        self.notify("search");
    }
}

impl Default for FuzzyFilter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::{cell::RefCell, rc::Rc};

    use crate::model::SongId;

    #[test]
    fn strictness() {
        let filter = FuzzyFilter::new();
        assert_eq!(filter.strictness(), gtk::FilterMatch::All);

        filter.set_search("foo");
        assert_eq!(filter.strictness(), gtk::FilterMatch::Some);

        filter.set_search("bar");
        assert_eq!(filter.strictness(), gtk::FilterMatch::Some);

        filter.set_search("");
        assert_eq!(filter.strictness(), gtk::FilterMatch::All);
    }

    #[test]
    fn match_() {
        let filter = FuzzyFilter::new();
        assert!(filter.match_(&Song::builder(&SongId::from("0"), "foo", "foo", "").build()));
        assert!(filter.match_(&Song::builder(&SongId::from("1"), "bar", "bar", "").build()));

        filter.set_search("foo");
        assert!(filter.match_(&Song::builder(&SongId::from("2"), "foo", "foo", "").build()));
        assert!(!filter.match_(&Song::builder(&SongId::from("3"), "bar", "bar", "").build()));

        filter.set_search("bar");
        assert!(!filter.match_(&Song::builder(&SongId::from("4"), "foo", "foo", "").build()));
        assert!(filter.match_(&Song::builder(&SongId::from("5"), "bar", "bar", "").build()));

        filter.set_search("");
        assert!(filter.match_(&Song::builder(&SongId::from("6"), "foo", "foo", "").build()));
        assert!(filter.match_(&Song::builder(&SongId::from("7"), "bar", "bar", "").build()));
    }

    #[test]
    fn changed() {
        let filter = FuzzyFilter::new();

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
