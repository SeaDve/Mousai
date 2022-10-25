use gtk::{gio, glib, prelude::*, subclass::prelude::*};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use std::cell::RefCell;

use super::external_link::{ExternalLink, ExternalLinkWrapper};

mod imp {
    use super::*;

    #[derive(Debug, Default)]
    pub struct ExternalLinkList(pub(super) RefCell<Vec<ExternalLinkWrapper>>);

    #[glib::object_subclass]
    impl ObjectSubclass for ExternalLinkList {
        const NAME: &'static str = "MsaiExternalLinkList";
        type Type = super::ExternalLinkList;
        type Interfaces = (gio::ListModel,);
    }

    impl ObjectImpl for ExternalLinkList {}

    impl ListModelImpl for ExternalLinkList {
        fn item_type(&self) -> glib::Type {
            ExternalLinkWrapper::static_type()
        }

        fn n_items(&self) -> u32 {
            self.0.borrow().len() as u32
        }

        fn item(&self, position: u32) -> Option<glib::Object> {
            self.obj()
                .get(position as usize)
                .map(|item| item.upcast::<glib::Object>())
        }
    }
}

glib::wrapper! {
    pub struct ExternalLinkList(ObjectSubclass<imp::ExternalLinkList>)
        @implements gio::ListModel;
}

impl ExternalLinkList {
    pub fn new(items: Vec<Box<dyn ExternalLink>>) -> Self {
        let obj = Self::default();
        obj.push_many(items);
        obj
    }

    pub fn push(&self, external_link: impl ExternalLink + 'static) {
        self.imp()
            .0
            .borrow_mut()
            .push(ExternalLinkWrapper::new(external_link));
        self.items_changed(self.n_items() - 1, 0, 1);
    }

    /// This is more efficient than [`ExternalLinkList::push`] since it emits `items-changed` only once
    pub fn push_many(&self, mut items: Vec<Box<dyn ExternalLink>>) {
        let first_appended_pos = self.imp().0.borrow().len();
        let n_added = items.len();

        {
            let mut inner = self.imp().0.borrow_mut();
            for item in items.drain(..) {
                inner.push(ExternalLinkWrapper::from_boxed(item));
            }
        }

        if n_added != 0 {
            self.items_changed(first_appended_pos as u32, 0, n_added as u32);
        }
    }

    pub fn get(&self, position: usize) -> Option<ExternalLinkWrapper> {
        self.imp().0.borrow().get(position).cloned()
    }

    pub fn is_empty(&self) -> bool {
        self.n_items() == 0
    }
}

impl Default for ExternalLinkList {
    fn default() -> Self {
        glib::Object::new(&[])
    }
}

impl Serialize for ExternalLinkList {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.imp()
            .0
            .borrow()
            .iter()
            .map(|item| item.inner())
            .collect::<Vec<_>>()
            .serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for ExternalLinkList {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let external_links: Vec<Box<dyn ExternalLink>> = Vec::deserialize(deserializer)?;
        Ok(ExternalLinkList::new(external_links))
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Serialize, Deserialize)]
    struct TestExternalLink;

    #[typetag::serde]
    impl ExternalLink for TestExternalLink {
        fn uri(&self) -> String {
            "Test".to_string()
        }

        fn name(&self) -> String {
            "Test".to_string()
        }

        fn tooltip_text(&self) -> String {
            "Test".to_string()
        }

        fn icon_name(&self) -> &'static str {
            ""
        }

        fn css_class(&self) -> &'static str {
            ""
        }
    }

    #[test]
    fn items_changed_push() {
        let list = ExternalLinkList::default();
        assert_eq!(list.n_items(), 0);

        list.connect_items_changed(|list, pos, removed, added| {
            assert_eq!(pos, 0);
            assert_eq!(removed, 0);
            assert_eq!(added, 1);
            assert_eq!(list.n_items(), 1);
        });

        list.push(TestExternalLink);
    }

    #[test]
    fn items_changed_push_had_something() {
        let list = ExternalLinkList::default();
        list.push(TestExternalLink);
        assert_eq!(list.n_items(), 1);

        list.connect_items_changed(|list, pos, removed, added| {
            assert_eq!(pos, 1);
            assert_eq!(removed, 0);
            assert_eq!(added, 1);
            assert_eq!(list.n_items(), 2);
        });

        list.push(TestExternalLink);
    }

    #[test]
    fn items_changed_push_many() {
        let list = ExternalLinkList::default();
        assert_eq!(list.n_items(), 0);

        list.connect_items_changed(|list, pos, removed, added| {
            assert_eq!(pos, 0);
            assert_eq!(removed, 0);
            assert_eq!(added, 2);
            assert_eq!(list.n_items(), 2);
        });

        list.push_many(vec![Box::new(TestExternalLink), Box::new(TestExternalLink)]);
    }

    #[test]
    fn items_changed_push_many_had_something() {
        let list = ExternalLinkList::default();
        list.push(TestExternalLink);
        assert_eq!(list.n_items(), 1);

        list.connect_items_changed(|list, pos, removed, added| {
            assert_eq!(pos, 1);
            assert_eq!(removed, 0);
            assert_eq!(added, 2);
            assert_eq!(list.n_items(), 3);
        });

        list.push_many(vec![Box::new(TestExternalLink), Box::new(TestExternalLink)]);
    }

    #[test]
    fn miscellaneous() {
        let list = ExternalLinkList::default();
        assert_eq!(list.item_type(), ExternalLinkWrapper::static_type());
        assert!(list.is_empty());

        list.push_many(vec![Box::new(TestExternalLink), Box::new(TestExternalLink)]);
        assert!(list.item(0).is_some());
        assert!(list.item(2).is_none());
    }
}
