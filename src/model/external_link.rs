use gtk::glib::{self, subclass::prelude::*};

use std::cell::OnceCell;

mod imp {
    use super::*;

    #[derive(Default)]
    pub struct ExternalLink {
        pub(super) key: OnceCell<String>,
        pub(super) value: OnceCell<String>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for ExternalLink {
        const NAME: &'static str = "MsaiExternalLink";
        type Type = super::ExternalLink;
    }

    impl ObjectImpl for ExternalLink {}
}

glib::wrapper! {
    pub struct ExternalLink(ObjectSubclass<imp::ExternalLink>);
}

impl ExternalLink {
    pub fn new(key: String, value: String) -> Self {
        let this: Self = glib::Object::new();
        this.imp().key.set(key).unwrap();
        this.imp().value.set(value).unwrap();
        this
    }

    pub fn key(&self) -> &str {
        self.imp().key.get().unwrap().as_str()
    }

    pub fn value(&self) -> &str {
        self.imp().value.get().unwrap().as_str()
    }
}
