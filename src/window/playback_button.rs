use gettextrs::gettext;
use gtk::{glib, prelude::*, subclass::prelude::*};

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
    use std::marker::PhantomData;

    #[derive(Default, glib::Properties, gtk::CompositeTemplate)]
    #[properties(wrapper_type = super::PlaybackButton)]
    #[template(resource = "/io/github/seadve/Mousai/ui/playback-button.ui")]
    pub struct PlaybackButton {
        /// State or mode
        #[property(get, set = Self::set_mode, explicit_notify, builder(PlaybackButtonMode::default()))]
        pub(super) mode: Cell<PlaybackButtonMode>,
        /// Name of the action to trigger when clicked
        #[property(get = Self::action_name, set = Self::set_action_name, explicit_notify)]
        pub(super) action_name: PhantomData<Option<glib::GString>>,

        #[template_child]
        pub(super) button: TemplateChild<gtk::Button>,
        #[template_child]
        pub(super) stack: TemplateChild<gtk::Stack>,
        #[template_child]
        pub(super) image_child: TemplateChild<gtk::Image>,
        #[template_child]
        pub(super) spinner_child: TemplateChild<gtk::Spinner>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for PlaybackButton {
        const NAME: &'static str = "MsaiPlaybackButton";
        type Type = super::PlaybackButton;
        type ParentType = gtk::Widget;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();

            klass.set_accessible_role(gtk::AccessibleRole::Button);
            klass.set_css_name("playbackbutton");
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for PlaybackButton {
        crate::derived_properties!();

        fn constructed(&self) {
            self.parent_constructed();

            let obj = self.obj();

            obj.update_ui();
        }

        fn dispose(&self) {
            self.dispose_template();
        }
    }

    impl WidgetImpl for PlaybackButton {}

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

        fn action_name(&self) -> Option<glib::GString> {
            self.button.action_name()
        }

        fn set_action_name(&self, action_name: Option<&str>) {
            self.button.set_action_name(action_name);
            self.obj().notify_action_name();
        }
    }
}

glib::wrapper! {
    pub struct PlaybackButton(ObjectSubclass<imp::PlaybackButton>)
        @extends gtk::Widget;
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
                imp.button.set_tooltip_text(Some(&gettext("Play")));
                imp.stack.set_visible_child(&imp.image_child.get());
                imp.spinner_child.set_spinning(false);
            }
            PlaybackButtonMode::Pause => {
                imp.image_child
                    .set_icon_name(Some("media-playback-pause-symbolic"));
                imp.button.set_tooltip_text(Some(&gettext("Pause")));
                imp.stack.set_visible_child(&imp.image_child.get());
                imp.spinner_child.set_spinning(false);
            }
            PlaybackButtonMode::Buffering => {
                imp.button.set_tooltip_text(None);
                imp.stack.set_visible_child(&imp.spinner_child.get());
                imp.spinner_child.set_spinning(true);
            }
        }
    }
}

impl Default for PlaybackButton {
    fn default() -> Self {
        Self::new()
    }
}
