use gettextrs::{gettext, ngettext};
use gtk::{
    glib::{self, clone, closure_local},
    prelude::*,
    subclass::prelude::*,
};
use once_cell::unsync::OnceCell;

use super::progress_icon::ProgressIcon;
use crate::{
    debug_assert_or_log,
    recognizer::{RecognizeResult, Recognizer},
};

mod imp {
    use super::*;
    use glib::subclass::Signal;
    use once_cell::sync::Lazy;

    #[derive(Debug, Default, gtk::CompositeTemplate)]
    #[template(resource = "/io/github/seadve/Mousai/ui/recognizer-status-button.ui")]
    pub struct RecognizerStatusButton {
        #[template_child]
        pub(super) stack: TemplateChild<gtk::Stack>,
        #[template_child]
        pub(super) recognizing_page: TemplateChild<adw::Bin>,
        #[template_child]
        pub(super) recognizing_progress_icon: TemplateChild<ProgressIcon>,
        #[template_child]
        pub(super) n_successful_recognition_page: TemplateChild<adw::Bin>,
        #[template_child]
        pub(super) n_successful_recognition_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub(super) no_network_page: TemplateChild<adw::Bin>,

        pub(super) recognizer: OnceCell<Recognizer>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for RecognizerStatusButton {
        const NAME: &'static str = "MsaiRecognizerStatusButton";
        type Type = super::RecognizerStatusButton;
        type ParentType = gtk::Widget;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();

            klass.install_action(
                "recognizer-status-button.request-show-saved-recordings",
                None,
                |obj, _, _| {
                    obj.emit_by_name::<()>("show-saved-recordings-requested", &[]);
                },
            );
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for RecognizerStatusButton {
        fn signals() -> &'static [glib::subclass::Signal] {
            static SIGNALS: Lazy<Vec<Signal>> =
                Lazy::new(|| vec![Signal::builder("show-saved-recordings-requested").build()]);

            SIGNALS.as_ref()
        }

        fn dispose(&self) {
            self.dispose_template();
        }
    }

    impl WidgetImpl for RecognizerStatusButton {}
}

glib::wrapper! {
     pub struct RecognizerStatusButton(ObjectSubclass<imp::RecognizerStatusButton>)
        @extends gtk::Widget;
}

impl RecognizerStatusButton {
    pub fn new() -> Self {
        glib::Object::new()
    }

    pub fn connect_show_saved_recordings_requested<F>(&self, f: F) -> glib::SignalHandlerId
    where
        F: Fn(&Self) + 'static,
    {
        self.connect_closure(
            "show-saved-recordings-requested",
            true,
            closure_local!(|obj: &Self| {
                f(obj);
            }),
        )
    }

    /// Must be called only once
    pub fn bind_recognizer(&self, recognizer: &Recognizer) {
        recognizer.connect_is_offline_mode_notify(clone!(@weak self as obj => move |_| {
            obj.update_ui();
        }));

        recognizer.connect_saved_recordings_changed(clone!(@weak self as obj => move |_| {
            obj.update_ui();
        }));

        self.imp().recognizer.set(recognizer.clone()).unwrap();

        self.update_ui();
    }

    fn update_ui(&self) {
        let imp = self.imp();

        let recognizer = self.imp().recognizer.get().unwrap();

        let is_offline_mode = recognizer.is_offline_mode();

        let saved_recordings = recognizer.saved_recordings();

        let recognized_saved_recordings = recognizer.peek_recognized_saved_recordings();
        let n_successful_recognition = recognized_saved_recordings
            .iter()
            .filter(|recording| {
                matches!(*recording.recognize_result(), Some(RecognizeResult::Ok(_)))
            })
            .count();

        if !is_offline_mode && saved_recordings.len() != recognized_saved_recordings.len() {
            self.set_visible(true);

            debug_assert_or_log!(saved_recordings.len() > recognized_saved_recordings.len());
            let n_recognizing = saved_recordings.len() - recognized_saved_recordings.len();
            self.set_tooltip_text(Some(&ngettext!(
                "Recognizing {} Song",
                "Recognizing {} Songs",
                n_recognizing as u32,
                n_recognizing,
            )));

            imp.recognizing_progress_icon.set_progress(
                recognized_saved_recordings.len() as f32 / saved_recordings.len() as f32,
            );

            imp.stack.set_visible_child(&imp.recognizing_page.get());
        } else if n_successful_recognition != 0 {
            self.set_visible(true);

            self.set_tooltip_text(Some(&ngettext(
                "Show New Song",
                "Show New Songs",
                n_successful_recognition as u32,
            )));

            imp.n_successful_recognition_label
                .set_label(&n_successful_recognition.to_string());

            imp.stack
                .set_visible_child(&imp.n_successful_recognition_page.get());
        } else if is_offline_mode {
            self.set_visible(true);

            self.set_tooltip_text(Some(&gettext("Offline Mode Enabled")));

            imp.stack.set_visible_child(&imp.no_network_page.get());
        } else {
            tracing::debug!(
                is_offline_mode,
                saved_recordings = saved_recordings.len(),
                recognized_saved_recordings = recognized_saved_recordings.len(),
                n_successful_recognition,
                "reached unhandled case"
            );
            self.set_visible(false);
        }
    }
}

impl Default for RecognizerStatusButton {
    fn default() -> Self {
        Self::new()
    }
}
