mod apple_music;
mod aud_d;
mod spotify;
mod youtube;

use gtk::glib::{self, subclass::prelude::*};
use once_cell::unsync::OnceCell;

use std::fmt;

pub use self::{
    apple_music::AppleMusicExternalLink, aud_d::AudDExternalLink, spotify::SpotifyExternalLink,
    youtube::YoutubeExternalLink,
};

#[typetag::serde]
pub trait ExternalLink: fmt::Debug {
    /// The uri to launch when this is activated
    fn uri(&self) -> String;

    /// The visible label of this in the UI
    fn name(&self) -> String;

    /// Text that will be shown when hovered in the UI
    fn tooltip_text(&self) -> String;

    /// Icon name to lookup that will be displayed in the UI
    fn icon_name(&self) -> &'static str;

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
    /// GObject wrapper for [`ExternalLink`]
    pub struct ExternalLinkWrapper(ObjectSubclass<imp::ExternalLinkWrapper>);
}

impl ExternalLinkWrapper {
    pub fn new(inner: impl ExternalLink + 'static) -> Self {
        let obj: Self = glib::Object::new();
        obj.imp().0.set(Box::new(inner)).unwrap();
        obj
    }

    pub fn from_boxed(inner: Box<dyn ExternalLink>) -> Self {
        let obj: Self = glib::Object::new();
        obj.imp().0.set(inner).unwrap();
        obj
    }

    pub fn inner(&self) -> &dyn ExternalLink {
        self.imp().0.get().unwrap().as_ref()
    }
}
