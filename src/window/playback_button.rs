use adw::prelude::*;
use gettextrs::gettext;
use gtk::{glib, subclass::prelude::*};

use std::cell::Cell;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, glib::Enum)]
#[enum_type(name = "MsaiPlaybackButtonMode")]
pub enum PlaybackButtonMode {
    #[default]
    Play,
    Pause,
    Buffering,
}

mod imp {
    use super::*;

    #[derive(Default, glib::Properties, gtk::CompositeTemplate)]
    #[properties(wrapper_type = super::PlaybackButton)]
    #[template(resource = "/io/github/seadve/Mousai/ui/playback-button.ui")]
    pub struct PlaybackButton {
        /// State or mode
        #[property(get, set = Self::set_mode, explicit_notify, builder(PlaybackButtonMode::default()))]
        pub(super) mode: Cell<PlaybackButtonMode>,

        #[template_child]
        pub(super) stack: TemplateChild<gtk::Stack>,
        #[template_child]
        pub(super) image_child: TemplateChild<gtk::Image>,
        #[template_child]
        pub(super) spinner_child: TemplateChild<adw::Spinner>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for PlaybackButton {
        const NAME: &'static str = "MsaiPlaybackButton";
        type Type = super::PlaybackButton;
        type ParentType = gtk::Button;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    #[glib::derived_properties]
    impl ObjectImpl for PlaybackButton {
        fn constructed(&self) {
            self.parent_constructed();

            let obj = self.obj();

            obj.update_ui();
        }
    }

    impl WidgetImpl for PlaybackButton {}
    impl ButtonImpl for PlaybackButton {}

    impl PlaybackButton {
        fn set_mode(&self, mode: PlaybackButtonMode) {
            let obj = self.obj();

            if mode == obj.mode() {
                return;
            }

            self.mode.set(mode);
            obj.update_ui();
            obj.notify_mode();
        }
    }
}

glib::wrapper! {
    pub struct PlaybackButton(ObjectSubclass<imp::PlaybackButton>)
        @extends gtk::Widget, gtk::Button;
}

impl PlaybackButton {
    pub fn new() -> Self {
        glib::Object::new()
    }

    fn update_ui(&self) {
        let imp = self.imp();

        match self.mode() {
            PlaybackButtonMode::Play => {
                imp.image_child
                    .set_icon_name(Some("media-playback-start-symbolic"));
                self.set_tooltip_text(Some(&gettext("Play")));
                imp.stack.set_visible_child(&imp.image_child.get());
            }
            PlaybackButtonMode::Pause => {
                imp.image_child
                    .set_icon_name(Some("media-playback-pause-symbolic"));
                self.set_tooltip_text(Some(&gettext("Pause")));
                imp.stack.set_visible_child(&imp.image_child.get());
            }
            PlaybackButtonMode::Buffering => {
                self.set_tooltip_text(None);
                imp.stack.set_visible_child(&imp.spinner_child.get());
            }
        }
    }
}
