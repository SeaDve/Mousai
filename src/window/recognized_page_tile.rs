use gtk::{
    gdk,
    glib::{self, clone, closure_local},
    prelude::*,
    subclass::prelude::*,
};

use std::cell::RefCell;

use super::song_tile::SongTile;
use crate::{date_time::DateTime, player::Player, song::Song};

mod imp {
    use super::*;
    use glib::{once_cell::sync::Lazy, subclass::Signal};
    use std::marker::PhantomData;

    #[derive(Default, glib::Properties, gtk::CompositeTemplate)]
    #[properties(wrapper_type = super::RecognizedPageTile)]
    #[template(resource = "/io/github/seadve/Mousai/ui/recognized-page-tile.ui")]
    pub struct RecognizedPageTile {
        #[property(get = Self::song, set = Self::set_song, construct_only)]
        pub(super) song: PhantomData<Song>,

        #[template_child]
        pub(super) last_heard_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub(super) song_tile: TemplateChild<SongTile>,

        pub(super) binding: RefCell<Option<glib::Binding>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for RecognizedPageTile {
        const NAME: &'static str = "MsaiRecognizedPageTile";
        type Type = super::RecognizedPageTile;
        type ParentType = gtk::Widget;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();

            klass.set_css_name("recognizedpagetile");
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    #[glib::derived_properties]
    impl ObjectImpl for RecognizedPageTile {
        fn signals() -> &'static [Signal] {
            static SIGNALS: Lazy<Vec<Signal>> =
                Lazy::new(|| vec![Signal::builder("activated").build()]);

            SIGNALS.as_ref()
        }

        fn constructed(&self) {
            self.parent_constructed();

            let obj = self.obj();

            let gesture_click = gtk::GestureClick::builder()
                .button(gdk::BUTTON_PRIMARY)
                .build();
            gesture_click.connect_released(clone!(@weak obj => move |gesture, _, x, y| {
                gesture.set_state(gtk::EventSequenceState::Claimed);
                if gesture.widget().contains(x, y) {
                    obj.emit_by_name::<()>("activated", &[]);
                }
            }));
            self.song_tile.add_controller(gesture_click);
        }

        fn dispose(&self) {
            if let Some(binding) = self.binding.take() {
                binding.unbind();
            }

            self.obj().unbind_player();

            self.dispose_template();
        }
    }

    impl WidgetImpl for RecognizedPageTile {}

    impl RecognizedPageTile {
        fn song(&self) -> Song {
            self.song_tile.song().unwrap()
        }

        fn set_song(&self, song: &Song) {
            if let Some(binding) = self.binding.take() {
                binding.unbind();
            }

            let binding = song
                .bind_property("last-heard", &self.last_heard_label.get(), "label")
                .transform_to(|_, last_heard: Option<DateTime>| {
                    Some(
                        last_heard.map_or_else(glib::GString::default, |last_heard| {
                            last_heard.to_local().fuzzy_display()
                        }),
                    )
                })
                .sync_create()
                .build();
            self.binding.replace(Some(binding));

            self.song_tile.set_song(song);
        }
    }
}

glib::wrapper! {
     pub struct RecognizedPageTile(ObjectSubclass<imp::RecognizedPageTile>)
        @extends gtk::Widget;
}

impl RecognizedPageTile {
    pub fn new(song: &Song) -> Self {
        glib::Object::builder().property("song", song).build()
    }

    pub fn connect_activated<F>(&self, f: F) -> glib::SignalHandlerId
    where
        F: Fn(&Self) + 'static,
    {
        self.connect_closure(
            "activated",
            true,
            closure_local!(|obj: &Self| {
                f(obj);
            }),
        )
    }

    // Must only be called once
    pub fn bind_player(&self, player: &Player) {
        self.imp().song_tile.bind_player(player);
    }

    pub fn unbind_player(&self) {
        self.imp().song_tile.unbind_player();
    }
}
