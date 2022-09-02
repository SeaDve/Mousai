use adw::prelude::*;
use gettextrs::gettext;
use gtk::{
    glib::{self, clone},
    subclass::prelude::*,
};
use once_cell::unsync::OnceCell;

use super::waveform::Waveform;
use crate::recognizer::{Recognizer, RecognizerState};

mod imp {
    use super::*;
    use gtk::CompositeTemplate;

    #[derive(Debug, Default, CompositeTemplate)]
    #[template(resource = "/io/github/seadve/Mousai/ui/recognizer-view.ui")]
    pub struct RecognizerView {
        #[template_child]
        pub(super) title: TemplateChild<gtk::Label>,
        #[template_child]
        pub(super) waveform: TemplateChild<Waveform>,

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
        fn dispose(&self, obj: &Self::Type) {
            while let Some(child) = obj.first_child() {
                child.unparent();
            }
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
        glib::Object::new(&[]).expect("Failed to create RecognizerView")
    }

    /// Must be only called once
    pub fn bind_recognizer(&self, recognizer: &Recognizer) {
        recognizer.connect_state_notify(clone!(@weak self as obj => move |_| {
            obj.update_ui();
        }));

        let audio_recorder = recognizer.audio_recorder();
        audio_recorder.connect_peak(clone!(@weak self as obj => move |_, peak| {
            obj.imp().waveform.push_peak(peak);
        }));
        audio_recorder.connect_stopped(clone!(@weak self as obj => move |_| {
            obj.imp().waveform.clear_peaks();
        }));

        self.imp().recognizer.set(recognizer.clone()).unwrap();

        self.update_ui();
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
