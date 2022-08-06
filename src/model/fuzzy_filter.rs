// SPDX-FileCopyrightText: 2022  John Toohey <john_t@mailo.com>
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
        pub search: RefCell<Option<String>>,
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
                        .flags(glib::ParamFlags::READWRITE | glib::ParamFlags::EXPLICIT_NOTIFY)
                        .build(),
                ]
            });

            PROPERTIES.as_ref()
        }

        fn set_property(
            &self,
            obj: &Self::Type,
            _id: usize,
            value: &glib::Value,
            pspec: &glib::ParamSpec,
        ) {
            match pspec.name() {
                "search" => {
                    let search = value.get::<Option<String>>().unwrap();
                    obj.set_search(search.as_deref());
                }
                _ => unimplemented!(),
            }
        }

        fn property(&self, obj: &Self::Type, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
            match pspec.name() {
                "search" => obj.search().to_value(),
                _ => unimplemented!(),
            }
        }
    }

    impl FilterImpl for FuzzyFilter {
        fn strictness(&self, _filter: &Self::Type) -> gtk::FilterMatch {
            if self
                .search
                .borrow()
                .as_ref()
                .filter(|s| !s.is_empty())
                .is_some()
            {
                gtk::FilterMatch::Some
            } else {
                gtk::FilterMatch::All
            }
        }

        fn match_(&self, _filter: &Self::Type, song: &glib::Object) -> bool {
            let song = song.downcast_ref::<Song>().unwrap();

            if let Some(search) = self.search.borrow().as_ref().filter(|s| !s.is_empty()) {
                FUZZY_MATCHER
                    .fuzzy_match(&song.search_term(), search)
                    .is_some()
            } else {
                true
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
        glib::Object::new(&[]).expect("Failed to create FuzzyFilter.")
    }

    pub fn search(&self) -> Option<String> {
        self.imp().search.borrow().clone()
    }

    pub fn set_search(&self, search: Option<&str>) {
        let old_search = self.search();
        let search = search.map(|s| s.to_lowercase());

        if old_search == search {
            return;
        }

        let change = match (&old_search, &search) {
            (Some(old), Some(new)) => {
                if old.starts_with(new) {
                    gtk::FilterChange::LessStrict
                } else if new.starts_with(old) {
                    gtk::FilterChange::MoreStrict
                } else {
                    gtk::FilterChange::Different
                }
            }
            (Some(..), None) => gtk::FilterChange::LessStrict,
            (None, Some(..)) => gtk::FilterChange::MoreStrict,
            (None, None) => return,
        };

        self.imp().search.replace(search);
        self.changed(dbg!(change));
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

        filter.set_search(Some("foo"));
        assert_eq!(filter.strictness(), gtk::FilterMatch::Some);

        filter.set_search(Some(""));
        assert_eq!(filter.strictness(), gtk::FilterMatch::All);

        filter.set_search(None);
        assert_eq!(filter.strictness(), gtk::FilterMatch::All);
    }

    #[test]
    fn match_() {
        let filter = FuzzyFilter::new();
        assert!(filter.match_(&Song::builder(&SongId::from("0"), "foo", "foo", "").build()));
        assert!(filter.match_(&Song::builder(&SongId::from("1"), "bar", "bar", "").build()));

        filter.set_search(Some("foo"));
        assert!(filter.match_(&Song::builder(&SongId::from("2"), "foo", "foo", "").build()));
        assert!(!filter.match_(&Song::builder(&SongId::from("3"), "bar", "bar", "").build()));

        filter.set_search(Some(""));
        assert!(filter.match_(&Song::builder(&SongId::from("4"), "foo", "foo", "").build()));
        assert!(filter.match_(&Song::builder(&SongId::from("5"), "bar", "bar", "").build()));

        filter.set_search(None);
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

        filter.set_search(Some(""));
        assert_eq!(
            calls_output.borrow_mut().pop().unwrap(),
            gtk::FilterChange::MoreStrict
        );

        filter.set_search(Some("foo"));
        assert_eq!(
            calls_output.borrow_mut().pop().unwrap(),
            gtk::FilterChange::MoreStrict
        );

        filter.set_search(Some("fo"));
        assert_eq!(
            calls_output.borrow_mut().pop().unwrap(),
            gtk::FilterChange::LessStrict
        );

        filter.set_search(Some("bar"));
        assert_eq!(
            calls_output.borrow_mut().pop().unwrap(),
            gtk::FilterChange::Different
        );

        filter.set_search(Some("bars"));
        assert_eq!(
            calls_output.borrow_mut().pop().unwrap(),
            gtk::FilterChange::MoreStrict
        );

        filter.set_search(None);
        assert_eq!(
            calls_output.borrow_mut().pop().unwrap(),
            gtk::FilterChange::LessStrict
        );
    }
}
