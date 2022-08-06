// SPDX-FileCopyrightText: 2022  John Toohey <john_t@mailo.com>
// SPDX-License-Identifier: GPL-3.0-or-later

use fuzzy_matcher::{skim::SkimMatcherV2, FuzzyMatcher};
use gtk::{glib, prelude::*, subclass::prelude::*};

use std::cell::RefCell;

use super::Song;

mod imp {
    use super::*;
    use once_cell::sync::Lazy;

    #[derive(Debug, Default)]
    pub struct FuzzySorter {
        pub search: RefCell<Option<String>>,
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

    impl SorterImpl for FuzzySorter {
        fn compare(
            &self,
            _sorter: &Self::Type,
            item_1: &glib::Object,
            item_2: &glib::Object,
        ) -> gtk::Ordering {
            let song_1 = item_1.downcast_ref::<Song>().unwrap();
            let song_2 = item_2.downcast_ref::<Song>().unwrap();

            if let Some(search) = self.search.borrow().as_ref().filter(|s| !s.is_empty()) {
                let matcher = SkimMatcherV2::default();
                let item1_score = matcher.fuzzy_match(&song_1.search_term(), search);
                let item2_score = matcher.fuzzy_match(&song_2.search_term(), search);
                item2_score.cmp(&item1_score).into()
            } else {
                song_2.last_heard().cmp(&song_1.last_heard()).into()
            }
        }

        fn order(&self, _sorter: &Self::Type) -> gtk::SorterOrder {
            if self
                .search
                .borrow()
                .as_ref()
                .filter(|s| !s.is_empty())
                .is_some()
            {
                gtk::SorterOrder::Partial
            } else {
                gtk::SorterOrder::Total
            }
        }
    }
}

glib::wrapper! {
    pub struct FuzzySorter(ObjectSubclass<imp::FuzzySorter>)
        @extends gtk::Sorter;

}

impl FuzzySorter {
    pub fn new() -> Self {
        glib::Object::new(&[]).expect("Failed to create `AmberolFuzzySorter`")
    }

    pub fn search(&self) -> Option<String> {
        self.imp().search.borrow().clone()
    }

    pub fn set_search(&self, search: Option<&str>) {
        if self.search().as_deref() == search {
            return;
        }

        self.imp().search.replace(search.map(|s| s.to_string()));
        self.changed(gtk::SorterChange::Different);
        self.notify("search");
    }
}

impl Default for FuzzySorter {
    fn default() -> Self {
        Self::new()
    }
}
