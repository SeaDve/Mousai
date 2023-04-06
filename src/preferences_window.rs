use adw::{prelude::*, subclass::prelude::*};
use gettextrs::gettext;
use gtk::glib::{self, clone};
use once_cell::unsync::OnceCell;

use crate::settings::{PreferredAudioSource, Settings};

impl PreferredAudioSource {
    fn from_position(index: u32) -> Self {
        match index {
            0 => Self::Microphone,
            1 => Self::DesktopAudio,
            _ => unreachable!(),
        }
    }

    fn as_position(self) -> u32 {
        match self {
            Self::Microphone => 0,
            Self::DesktopAudio => 1,
        }
    }
}

mod imp {
    use super::*;

    #[derive(Debug, Default, glib::Properties, gtk::CompositeTemplate)]
    #[properties(wrapper_type = super::PreferencesWindow)]
    #[template(resource = "/io/github/seadve/Mousai/ui/preferences-window.ui")]
    pub struct PreferencesWindow {
        #[property(get, set, construct_only)]
        pub(super) settings: OnceCell<Settings>,

        #[template_child]
        pub(super) preferred_audio_source_row: TemplateChild<adw::ComboRow>,
        #[template_child]
        pub(super) aud_d_api_token_row: TemplateChild<adw::EntryRow>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for PreferencesWindow {
        const NAME: &'static str = "MsaiPreferencesWindow";
        type Type = super::PreferencesWindow;
        type ParentType = adw::PreferencesWindow;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for PreferencesWindow {
        fn constructed(&self) {
            self.parent_constructed();

            let obj = self.obj();

            if tracing::enabled!(tracing::Level::TRACE) {
                obj.settings().connect_changed(None, |settings, key| {
                    tracing::trace!("Settings changed: {} = {:?}", key, settings.value(key));
                });
            }

            obj.setup_rows();
        }

        fn dispose(&self) {
            self.dispose_template();
        }

        crate::derived_properties!();
    }

    impl WidgetImpl for PreferencesWindow {}
    impl WindowImpl for PreferencesWindow {}
    impl AdwWindowImpl for PreferencesWindow {}
    impl PreferencesWindowImpl for PreferencesWindow {}
}

glib::wrapper! {
    pub struct PreferencesWindow(ObjectSubclass<imp::PreferencesWindow>)
        @extends gtk::Widget, gtk::Window, gtk::ApplicationWindow, adw::ApplicationWindow;
}

impl PreferencesWindow {
    pub fn new(settings: &Settings) -> Self {
        glib::Object::builder()
            .property("settings", settings)
            .build()
    }

    fn setup_rows(&self) {
        let imp = self.imp();

        let settings = self.settings();

        imp.preferred_audio_source_row
            .set_model(Some(&gtk::StringList::new(&[
                &gettext("Microphone"),
                &gettext("Desktop Audio"),
            ])));
        imp.preferred_audio_source_row
            .set_selected(settings.preferred_audio_source().as_position());
        imp.preferred_audio_source_row.connect_selected_notify(
            clone!(@weak self as obj => move |provider_row| {
                obj.settings()
                    .set_preferred_audio_source(PreferredAudioSource::from_position(
                        provider_row.selected(),
                    ));
            }),
        );

        imp.aud_d_api_token_row
            .set_text(&settings.aud_d_api_token());
        imp.aud_d_api_token_row
            .connect_apply(clone!(@weak self as obj => move |row| {
                obj.settings().set_aud_d_api_token(&row.text());
            }));
    }
}
