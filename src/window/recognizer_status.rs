use gtk::{
    glib::{self, clone, closure_local},
    prelude::*,
    subclass::prelude::*,
};

use std::cell::OnceCell;

use super::progress_icon::ProgressIcon;
use crate::{i18n::gettext_f, recognizer::Recognizer};

// TODO
// - Maybe drop the separate button to show result?
// - Show "n queued recordings will be recognized once back online" in the progress icon when offline
// - Show more detailed progress with error messages

mod imp {
    use super::*;
    use glib::{once_cell::sync::Lazy, subclass::Signal};

    #[derive(Default, gtk::CompositeTemplate)]
    #[template(resource = "/io/github/seadve/Mousai/ui/recognizer-status.ui")]
    pub struct RecognizerStatus {
        #[template_child]
        pub(super) progress_icon_revealer: TemplateChild<gtk::Revealer>,
        #[template_child]
        pub(super) progress_icon: TemplateChild<ProgressIcon>,
        #[template_child]
        pub(super) offline_mode_icon_revealer: TemplateChild<gtk::Revealer>,
        #[template_child]
        pub(super) show_results_button_revealer: TemplateChild<gtk::Revealer>,
        #[template_child]
        pub(super) show_results_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub(super) show_results_button_label: TemplateChild<gtk::Label>,

        pub(super) recognizer: OnceCell<Recognizer>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for RecognizerStatus {
        const NAME: &'static str = "MsaiRecognizerStatus";
        type Type = super::RecognizerStatus;
        type ParentType = gtk::Widget;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for RecognizerStatus {
        fn signals() -> &'static [glib::subclass::Signal] {
            static SIGNALS: Lazy<Vec<Signal>> =
                Lazy::new(|| vec![Signal::builder("show-results-requested").build()]);

            SIGNALS.as_ref()
        }

        fn constructed(&self) {
            self.parent_constructed();

            let obj = self.obj();

            self.show_results_button
                .connect_clicked(clone!(@weak obj => move |_| {
                    obj.emit_by_name::<()>("show-results-requested", &[]);
                }));
        }

        fn dispose(&self) {
            self.dispose_template();
        }
    }

    impl WidgetImpl for RecognizerStatus {}
}

glib::wrapper! {
     pub struct RecognizerStatus(ObjectSubclass<imp::RecognizerStatus>)
        @extends gtk::Widget;
}

impl RecognizerStatus {
    pub fn new() -> Self {
        glib::Object::new()
    }

    pub fn connect_show_results_requested<F>(&self, f: F) -> glib::SignalHandlerId
    where
        F: Fn(&Self) + 'static,
    {
        self.connect_closure(
            "show-results-requested",
            true,
            closure_local!(|obj: &Self| {
                f(obj);
            }),
        )
    }

    /// Must be called only once
    pub fn bind_recognizer(&self, recognizer: &Recognizer) {
        recognizer.connect_is_offline_mode_notify(clone!(@weak self as obj => move |_| {
            obj.update_offline_mode_ui();
        }));

        recognizer.saved_recordings().connect_items_changed(
            clone!(@weak self as obj => move |_, _, _, _| {
                obj.update_progress_and_show_results_ui();
            }),
        );

        self.imp().recognizer.set(recognizer.clone()).unwrap();

        self.update_offline_mode_ui();
        self.update_progress_and_show_results_ui();
    }

    fn update_offline_mode_ui(&self) {
        let imp = self.imp();

        let is_offline_mode = imp.recognizer.get().unwrap().is_offline_mode();
        imp.offline_mode_icon_revealer
            .set_reveal_child(is_offline_mode);
    }

    fn update_progress_and_show_results_ui(&self) {
        let imp = self.imp();

        let recognizer = self.imp().recognizer.get().unwrap();

        let total = recognizer.saved_recordings().n_items() as usize;
        let n_recognized = recognizer.peek_recognized_saved_recordings().len();
        debug_assert!(
            n_recognized <= total,
            "total must always be greater or equal than n_recognized"
        );

        let n_successful = self
            .imp()
            .recognizer
            .get()
            .unwrap()
            .peek_recognized_saved_recordings()
            .iter()
            .filter(|recording| {
                recording
                    .recognize_result()
                    .map_or(false, |res| res.0.is_ok())
            })
            .count();
        let n_failed = n_recognized - n_successful;

        imp.progress_icon.set_tooltip_text(Some(&gettext_f(
            // Translators: Do NOT translate the contents between '{' and '}', this is a variable name.
            "Recognized {n_successful} Out Of {total_minus_failed}",
            &[
                ("n_successful", &n_successful.to_string()),
                ("total_minus_failed", &(total - n_failed).to_string()),
            ],
        )));

        let progress = if total - n_failed == 0 {
            1.0
        } else {
            n_successful as f64 / (total - n_failed) as f64
        };
        imp.progress_icon.set_progress(progress);

        let has_unfinished = total != n_recognized;
        imp.progress_icon_revealer.set_reveal_child(has_unfinished);

        imp.show_results_button_revealer
            .set_reveal_child(n_successful != 0);

        imp.show_results_button_label
            .set_label(&n_successful.to_string());
    }
}

impl Default for RecognizerStatus {
    fn default() -> Self {
        Self::new()
    }
}
