use anyhow::Error;
use gettextrs::gettext;
use gtk::{gdk, glib, prelude::*, subclass::prelude::*};
use once_cell::unsync::OnceCell;

use crate::{
    debug_unreachable_or_log,
    model::{ExternalLink, ExternalLinkKey},
    utils,
};

mod imp {
    use super::*;

    #[derive(Debug, Default, glib::Properties, gtk::CompositeTemplate)]
    #[properties(wrapper_type = super::ExternalLinkTile)]
    #[template(resource = "/io/github/seadve/Mousai/ui/external-link-tile.ui")]
    pub struct ExternalLinkTile {
        /// Link shown by Self
        #[property(get, set, construct_only)]
        pub(super) external_link: OnceCell<ExternalLink>,

        #[template_child]
        pub(super) image: TemplateChild<gtk::Image>,
        #[template_child]
        pub(super) label: TemplateChild<gtk::Label>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for ExternalLinkTile {
        const NAME: &'static str = "MsaiExternalLinkTile";
        type Type = super::ExternalLinkTile;
        type ParentType = gtk::FlowBoxChild;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for ExternalLinkTile {
        crate::derived_properties!();

        fn constructed(&self) {
            self.parent_constructed();

            let obj = self.obj();

            let link = obj.external_link();

            match ExternalLinkKey::from_str(link.key()) {
                Some(key) => match key {
                    ExternalLinkKey::AppleMusicUrl => {
                        self.label.set_label(&gettext("Apple Music"));
                        obj.set_tooltip_text(Some(&gettext("Browse on Apple Music")));
                        self.image.set_icon_name(Some("music-note-symbolic"));
                        obj.add_css_class("applemusic");
                    }
                    ExternalLinkKey::AudDUrl => {
                        self.label.set_label(&gettext("AudD"));
                        obj.set_tooltip_text(Some(&gettext("Browse on AudD")));
                        self.image.set_icon_name(Some("microphone-symbolic"));
                        obj.add_css_class("audd");
                    }
                    ExternalLinkKey::SpotifyUrl => {
                        self.label.set_label(&gettext("Spotify"));
                        obj.set_tooltip_text(Some(&gettext("Listen on Spotify")));
                        self.image.set_icon_name(Some("network-wireless-symbolic"));
                        obj.add_css_class("spotify");
                    }
                    ExternalLinkKey::YoutubeSearchTerm => {
                        self.label.set_label(&gettext("YouTube"));
                        obj.set_tooltip_text(Some(&gettext("Search on YouTube")));
                        self.image
                            .set_icon_name(Some("media-playback-start-symbolic"));
                        obj.add_css_class("youtube");
                    }
                },
                None => {
                    tracing::warn!("Unhandled external link key `{}`", link.key());
                    obj.set_visible(false);
                }
            }
        }
    }

    impl WidgetImpl for ExternalLinkTile {}
    impl FlowBoxChildImpl for ExternalLinkTile {}
}

glib::wrapper! {
    pub struct ExternalLinkTile(ObjectSubclass<imp::ExternalLinkTile>)
        @extends gtk::Widget, gtk::FlowBoxChild;
}

impl ExternalLinkTile {
    pub fn new(external_link: &ExternalLink) -> Self {
        glib::Object::builder()
            .property("external-link", external_link)
            .build()
    }

    pub async fn handle_activation(&self) {
        let link = self.external_link();
        let raw_key = link.key();
        let raw_value = link.value();

        let Some(key) = ExternalLinkKey::from_str(raw_key) else {
                debug_unreachable_or_log!("activated a supposed non-visible external link tile with key `{}`", raw_key);
                return;
            };

        let uri = match key {
            ExternalLinkKey::AppleMusicUrl
            | ExternalLinkKey::AudDUrl
            | ExternalLinkKey::SpotifyUrl => raw_value.to_string(),
            ExternalLinkKey::YoutubeSearchTerm => {
                let escaped_search_term = glib::Uri::escape_string(raw_value, None, true);
                format!(
                    "https://www.youtube.com/results?search_query={}",
                    escaped_search_term
                )
            }
        };

        if let Err(err) = glib::Uri::is_valid(&uri, glib::UriFlags::ENCODED) {
            tracing::warn!("Trying to launch an invalid Uri: {:?}", err);
        }

        if let Err(err) = gtk::show_uri_full_future(
            self.root()
                .map(|root| root.downcast::<gtk::Window>().unwrap())
                .as_ref(),
            &uri,
            gdk::CURRENT_TIME,
        )
        .await
        {
            tracing::warn!("Failed to launch default for uri `{uri}`: {:?}", err);
            utils::app_instance()
                .add_toast_error(&Error::msg(gettext!("Failed to launch {}", raw_key)));
        }
    }
}
