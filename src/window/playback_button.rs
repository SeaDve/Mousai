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
    use gtk::CompositeTemplate;
    use once_cell::sync::Lazy;

    #[derive(Debug, Default, CompositeTemplate)]
    #[template(resource = "/io/github/seadve/Mousai/ui/playback-button.ui")]
    pub struct PlaybackButton {
        #[template_child]
        pub(super) button: TemplateChild<gtk::Button>,
        #[template_child]
        pub(super) stack: TemplateChild<gtk::Stack>,
        #[template_child]
        pub(super) image_child: TemplateChild<gtk::Image>,
        #[template_child]
        pub(super) spinner_child: TemplateChild<gtk::Spinner>,

        pub(super) mode: Cell<PlaybackButtonMode>,
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
        fn properties() -> &'static [glib::ParamSpec] {
            static PROPERTIES: Lazy<Vec<glib::ParamSpec>> = Lazy::new(|| {
                vec![
                    // State or mode of the button
                    glib::ParamSpecEnum::builder("mode", PlaybackButtonMode::static_type())
                        .default_value(PlaybackButtonMode::default() as i32)
                        .flags(glib::ParamFlags::READWRITE | glib::ParamFlags::EXPLICIT_NOTIFY)
                        .build(),
                    // Name of the action to trigger when clicked
                    glib::ParamSpecString::builder("action-name")
                        .flags(glib::ParamFlags::READWRITE | glib::ParamFlags::EXPLICIT_NOTIFY)
                        .build(),
                ]
            });

            PROPERTIES.as_ref()
        }

        fn set_property(
            &self,
            obj: &Self::Type,
            _id: usize,
            value: &glib::Value,
            pspec: &glib::ParamSpec,
        ) {
            match pspec.name() {
                "mode" => {
                    let mode = value.get().unwrap();
                    obj.set_mode(mode);
                }
                "action-name" => {
                    let action_name = value.get().unwrap();
                    obj.set_action_name(action_name);
                }
                _ => unimplemented!(),
            }
        }

        fn property(&self, obj: &Self::Type, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
            match pspec.name() {
                "mode" => obj.mode().to_value(),
                "action-name" => obj.action_name().to_value(),
                _ => unimplemented!(),
            }
        }

        fn constructed(&self, obj: &Self::Type) {
            self.parent_constructed(obj);

            obj.update_ui();
        }

        fn dispose(&self, obj: &Self::Type) {
            while let Some(child) = obj.first_child() {
                child.unparent();
            }
        }
    }

    impl WidgetImpl for PlaybackButton {}
}

glib::wrapper! {
    pub struct PlaybackButton(ObjectSubclass<imp::PlaybackButton>)
        @extends gtk::Widget;
}

impl PlaybackButton {
    pub fn new() -> Self {
        glib::Object::new(&[]).expect("Failed to create PlaybackButton")
    }

    pub fn set_mode(&self, mode: PlaybackButtonMode) {
        if mode == self.mode() {
            return;
        }

        self.imp().mode.set(mode);
        self.update_ui();
        self.notify("mode");
    }

    pub fn mode(&self) -> PlaybackButtonMode {
        self.imp().mode.get()
    }

    pub fn set_action_name(&self, action_name: Option<&str>) {
        self.imp().button.set_action_name(action_name);
        self.notify("action-name");
    }

    pub fn action_name(&self) -> Option<glib::GString> {
        self.imp().button.action_name()
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
