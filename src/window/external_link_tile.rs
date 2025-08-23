use gettextrs::gettext;
use gtk::{gio, glib, prelude::*, subclass::prelude::*};

use std::{cell::OnceCell, str::FromStr};

use crate::{
    Application, external_link::ExternalLink, external_links::ExternalLinkKey, i18n::gettext_f,
};

mod imp {
    use super::*;

    #[derive(Default, glib::Properties, gtk::CompositeTemplate)]
    #[properties(wrapper_type = super::ExternalLinkTile)]
    #[template(resource = "/io/github/seadve/Mousai/ui/external_link_tile.ui")]
    pub struct ExternalLinkTile {
        /// Link shown by Self
        #[property(get, set, construct_only)]
        pub(super) external_link: OnceCell<ExternalLink>,

        #[template_child]
        pub(super) label: TemplateChild<gtk::Label>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for ExternalLinkTile {
        const NAME: &'static str = "MousaiExternalLinkTile";
        type Type = super::ExternalLinkTile;
        type ParentType = gtk::FlowBoxChild;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    #[glib::derived_properties]
    impl ObjectImpl for ExternalLinkTile {
        fn constructed(&self) {
            self.parent_constructed();

            let obj = self.obj();

            let link = obj.external_link();
            let raw_key = link.key();

            let Ok(key) = ExternalLinkKey::from_str(raw_key) else {
                unreachable!(
                    "external link tile should not be constructed with an unhandleable key `{}`",
                    raw_key
                );
            };

            match key {
                ExternalLinkKey::AppleMusicUrl => {
                    self.label.set_label(&gettext("Apple Music"));
                    obj.set_tooltip_text(Some(&gettext("Browse on Apple Music")));
                    obj.add_css_class("applemusic");
                }
                ExternalLinkKey::AudDUrl => {
                    self.label.set_label(&gettext("AudD"));
                    obj.set_tooltip_text(Some(&gettext("Browse on AudD")));
                    obj.add_css_class("audd");
                }
                ExternalLinkKey::SpotifyUrl => {
                    self.label.set_label(&gettext("Spotify"));
                    obj.set_tooltip_text(Some(&gettext("Listen on Spotify")));
                    obj.add_css_class("spotify");
                }
                ExternalLinkKey::YoutubeSearchTerm => {
                    self.label.set_label(&gettext("YouTube"));
                    obj.set_tooltip_text(Some(&gettext("Search on YouTube")));
                    obj.add_css_class("youtube");
                }
            }
        }
    }

    impl WidgetImpl for ExternalLinkTile {}
    impl FlowBoxChildImpl for ExternalLinkTile {}
}

glib::wrapper! {
    pub struct ExternalLinkTile(ObjectSubclass<imp::ExternalLinkTile>)
        @extends gtk::Widget, gtk::FlowBoxChild,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl ExternalLinkTile {
    pub fn new(external_link: &ExternalLink) -> Self {
        glib::Object::builder()
            .property("external-link", external_link)
            .build()
    }

    pub fn can_handle(link: &ExternalLink) -> bool {
        ExternalLinkKey::from_str(link.key())
            .inspect_err(|_| {
                tracing::warn!("Can't handle external link key `{}`", link.key());
            })
            .is_ok()
    }

    pub fn handle_activation(&self) {
        let link = self.external_link();
        let raw_key = link.key().to_string();
        let raw_value = link.value();

        let Ok(key) = ExternalLinkKey::from_str(&raw_key) else {
            unreachable!(
                "external link tile with an unhandleable key `{}` should not have been constructed and thus activated",
                raw_key
            );
        };

        let uri = match key {
            ExternalLinkKey::AppleMusicUrl
            | ExternalLinkKey::AudDUrl
            | ExternalLinkKey::SpotifyUrl => raw_value.to_string(),
            ExternalLinkKey::YoutubeSearchTerm => {
                format!(
                    "https://www.youtube.com/results?search_query={}",
                    glib::Uri::escape_string(raw_value, None, true)
                )
            }
        };

        if let Err(err) = glib::Uri::is_valid(&uri, glib::UriFlags::ENCODED) {
            tracing::warn!("Trying to launch an invalid Uri: {:?}", err);
        }

        gtk::UriLauncher::new(&uri).launch(
            self.root()
                .map(|root| root.downcast::<gtk::Window>().unwrap())
                .as_ref(),
            gio::Cancellable::NONE,
            move |res| {
                if let Err(err) = res {
                    tracing::warn!("Failed to launch default for uri `{}`: {:?}", uri, err);
                    Application::get().add_message_toast(&gettext_f(
                        // Translators: Do NOT translate the contents between '{' and '}', this is a variable name.
                        "Failed to launch {key}",
                        &[("key", &raw_key)],
                    ));
                }
            },
        );
    }
}
