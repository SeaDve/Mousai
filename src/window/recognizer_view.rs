use adw::prelude::*;
use gettextrs::gettext;
use gtk::{
    glib::{self, clone},
    subclass::prelude::*,
};
use once_cell::unsync::OnceCell;

use std::cell::RefCell;

use super::waveform::Waveform;
use crate::{
    audio_recording::AudioRecording,
    recognizer::{Recognizer, RecognizerState},
};

mod imp {
    use super::*;
    use glib::WeakRef;
    use gtk::CompositeTemplate;

    #[derive(Debug, Default, CompositeTemplate)]
    #[template(resource = "/io/github/seadve/Mousai/ui/recognizer-view.ui")]
    pub struct RecognizerView {
        #[template_child]
        pub(super) title: TemplateChild<gtk::Label>,
        #[template_child]
        pub(super) waveform: TemplateChild<Waveform>,
        #[template_child]
        pub(super) offline_mode_status: TemplateChild<gtk::Label>,

        pub(super) recording_signal_handler:
            RefCell<Option<(WeakRef<AudioRecording>, glib::SignalHandlerId)>>,
        pub(super) recognizing_animation: OnceCell<adw::TimedAnimation>,
        pub(super) recognizer: OnceCell<Recognizer>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for RecognizerView {
        const NAME: &'static str = "MsaiRecognizerView";
        type Type = super::RecognizerView;
        type ParentType = gtk::Widget;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for RecognizerView {
        fn dispose(&self) {
            self.dispose_template();
        }
    }

    impl WidgetImpl for RecognizerView {}
}

glib::wrapper! {
    pub struct RecognizerView(ObjectSubclass<imp::RecognizerView>)
        @extends gtk::Widget;
}

impl RecognizerView {
    pub fn new() -> Self {
        glib::Object::new()
    }

    /// Must be only called once
    pub fn bind_recognizer(&self, recognizer: &Recognizer) {
        recognizer.connect_offline_mode_notify(clone!(@weak self as obj => move |_| {
            obj.update_offline_mode_ui();
        }));

        recognizer.connect_state_notify(clone!(@weak self as obj => move |_| {
            obj.update_ui();
        }));

        recognizer.connect_recording_notify(clone!(@weak self as obj => move |_| {
            obj.on_recognizer_recording_notify();
        }));

        self.imp().recognizer.set(recognizer.clone()).unwrap();

        self.update_offline_mode_ui();
        self.update_ui();
    }

    fn on_recognizer_recording_notify(&self) {
        let imp = self.imp();
        let handlers = imp.recording_signal_handler.take();

        imp.waveform.clear_peaks();

        if let Some((recording, handler_id)) = handlers {
            if let Some(recording) = recording.upgrade() {
                recording.disconnect(handler_id);
            }
        }

        if let Some(recording) = self.recognizer().recording() {
            let handler_id = recording.connect_peak(clone!(@weak self as obj => move |_, peak| {
                obj.imp().waveform.push_peak(peak);
            }));
            imp.recording_signal_handler
                .replace(Some((recording.downgrade(), handler_id)));
        }
    }

    fn recognizer(&self) -> &Recognizer {
        self.imp().recognizer.get_or_init(|| {
            tracing::error!("Recognizer was not bound in RecognizerView. Creating a default one.");
            Recognizer::default()
        })
    }

    fn recognizing_animation(&self) -> &adw::TimedAnimation {
        let imp = self.imp();
        imp.recognizing_animation.get_or_init(|| {
            adw::TimedAnimation::builder()
                .widget(&imp.waveform.get())
                .value_from(0.0)
                .value_to(0.8)
                .duration(1500)
                .target(&adw::CallbackAnimationTarget::new(
                    clone!(@weak self as obj => move |value| {
                        obj.imp().waveform.push_peak(value);
                    }),
                ))
                .easing(adw::Easing::EaseOutBack)
                .repeat_count(u32::MAX)
                .alternate(true)
                .build()
        })
    }

    fn update_offline_mode_ui(&self) {
        let imp = self.imp();
        let is_offline_mode = self.recognizer().is_offline_mode();

        imp.offline_mode_status.set_visible(is_offline_mode);

        if is_offline_mode {
            imp.waveform.add_css_class("red");
        } else {
            imp.waveform.remove_css_class("red");
        }
    }

    fn update_ui(&self) {
        let imp = self.imp();

        match self.recognizer().state() {
            RecognizerState::Listening => {
                imp.waveform.clear_peaks();
                self.recognizing_animation().pause();
                imp.title.set_label(&gettext("Listening…"));
            }
            RecognizerState::Recognizing => {
                imp.waveform.clear_peaks();
                self.recognizing_animation().play();
                imp.title.set_label(&gettext("Recognizing…"));
            }
            RecognizerState::Null => {
                imp.waveform.clear_peaks();
                self.recognizing_animation().pause();
            }
        }
    }
}

impl Default for RecognizerView {
    fn default() -> Self {
        Self::new()
    }
}
