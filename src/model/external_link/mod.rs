mod aud_d;
mod spotify;
mod youtube;

use gtk::glib::{self, subclass::prelude::*};
use once_cell::unsync::OnceCell;

pub use self::{
    aud_d::AudDExternalLink, spotify::SpotifyExternalLink, youtube::YoutubeExternalLink,
};

#[typetag::serde(tag = "type")]
pub trait ExternalLink: std::fmt::Debug {
    /// This will be called when the link is activated
    fn activate(&self);

    /// The visible label of this in the UI
    fn name(&self) -> String;

    /// Text that will be shown when hovered in the UI
    fn tooltip_text(&self) -> String;

    /// The css class use for the widget of this
    fn css_class(&self) -> &'static str;
}

mod imp {
    use super::*;

    #[derive(Debug, Default)]
    pub struct ExternalLinkWrapper(pub(super) OnceCell<Box<dyn ExternalLink>>);

    #[glib::object_subclass]
    impl ObjectSubclass for ExternalLinkWrapper {
        const NAME: &'static str = "MsaiExternalLinkWrapper";
        type Type = super::ExternalLinkWrapper;
    }

    impl ObjectImpl for ExternalLinkWrapper {}
}

glib::wrapper! {
    /// GObject wrapper for [`ExternalLink`](ExternalLink)
    pub struct ExternalLinkWrapper(ObjectSubclass<imp::ExternalLinkWrapper>);
}

impl ExternalLinkWrapper {
    pub fn new(inner: impl ExternalLink + 'static) -> Self {
        let obj: Self = glib::Object::new(&[]).expect("Failed to create ExternalLinkWrapper.");
        obj.imp().0.set(Box::new(inner)).unwrap();
        obj
    }

    pub fn from_boxed(inner: Box<dyn ExternalLink>) -> Self {
        let obj: Self = glib::Object::new(&[]).expect("Failed to create ExternalLinkWrapper.");
        obj.imp().0.set(inner).unwrap();
        obj
    }

    pub fn inner(&self) -> &dyn ExternalLink {
        self.imp().0.get().unwrap().as_ref()
    }
}
