use gtk::{
    gdk,
    glib::{self, clone, closure_local},
    prelude::*,
    subclass::prelude::*,
};

use std::cell::RefCell;

use super::song_tile::SongTile;
use crate::{core::DateTime, model::Song, player::Player};

mod imp {
    use super::*;
    use glib::subclass::Signal;
    use once_cell::sync::Lazy;
    use std::marker::PhantomData;

    #[derive(Debug, Default, glib::Properties, gtk::CompositeTemplate)]
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

    impl ObjectImpl for RecognizedPageTile {
        crate::derived_properties!();

        fn signals() -> &'static [Signal] {
            static SIGNALS: Lazy<Vec<Signal>> =
                Lazy::new(|| vec![Signal::builder("activated").build()]);

            SIGNALS.as_ref()
        }

        fn constructed(&self) {
            self.parent_constructed();

            let obj = self.obj();

            let gesture_click = gtk::GestureClick::builder()
                .touch_only(false)
                .button(gdk::BUTTON_PRIMARY)
                .propagation_phase(gtk::PropagationPhase::Bubble)
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
            self.binding.take().unwrap().unbind();
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
            let binding = song
                .bind_property("last-heard", &self.last_heard_label.get(), "label")
                .transform_to(|_, last_heard: DateTime| Some(last_heard.fuzzy_display()))
                .sync_create()
                .build();
            self.binding.replace(Some(binding));
            self.song_tile.set_song(Some(song.clone()));
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
