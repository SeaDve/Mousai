use adw::{prelude::*, subclass::prelude::*};
use gettextrs::gettext;
use gtk::glib::{self, clone};

use std::cell::OnceCell;

use crate::settings::{AudioSourceType, Settings};

impl AudioSourceType {
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

    #[derive(Default, glib::Properties, gtk::CompositeTemplate)]
    #[properties(wrapper_type = super::PreferencesDialog)]
    #[template(resource = "/io/github/seadve/Mousai/ui/preferences-dialog.ui")]
    pub struct PreferencesDialog {
        #[property(get, set, construct_only)]
        pub(super) settings: OnceCell<Settings>,

        #[template_child]
        pub(super) audio_source_type_row: TemplateChild<adw::ComboRow>,
        #[template_child]
        pub(super) aud_d_api_token_row: TemplateChild<adw::EntryRow>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for PreferencesDialog {
        const NAME: &'static str = "MsaiPreferencesDialog";
        type Type = super::PreferencesDialog;
        type ParentType = adw::PreferencesDialog;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    #[glib::derived_properties]
    impl ObjectImpl for PreferencesDialog {
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
    }

    impl WidgetImpl for PreferencesDialog {}
    impl AdwDialogImpl for PreferencesDialog {}
    impl PreferencesDialogImpl for PreferencesDialog {}
}

glib::wrapper! {
    pub struct PreferencesDialog(ObjectSubclass<imp::PreferencesDialog>)
        @extends gtk::Widget, adw::Dialog, adw::PreferencesDialog;
}

impl PreferencesDialog {
    pub fn new(settings: &Settings) -> Self {
        glib::Object::builder()
            .property("settings", settings)
            .build()
    }

    pub fn focus_aud_d_api_token_row(&self) -> bool {
        self.imp().aud_d_api_token_row.grab_focus()
    }

    fn setup_rows(&self) {
        let imp = self.imp();

        let settings = self.settings();

        imp.audio_source_type_row
            .set_model(Some(&gtk::StringList::new(&[
                &gettext("Microphone"),
                &gettext("Desktop Audio"),
            ])));
        imp.audio_source_type_row
            .set_selected(settings.audio_source_type().as_position());
        imp.audio_source_type_row.connect_selected_notify(
            clone!(@weak self as obj => move |provider_row| {
                obj.settings()
                    .set_audio_source_type(AudioSourceType::from_position(
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
