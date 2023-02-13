use gettextrs::ngettext;
use gtk::{
    glib::{self, clone, closure_local},
    prelude::*,
    subclass::prelude::*,
};
use once_cell::unsync::OnceCell;

use std::cell::{Cell, RefCell};

use super::{recognized_page_tile::RecognizedPageTile, AdaptiveMode};
use crate::{model::Song, player::Player};

mod imp {
    use super::*;
    use glib::{subclass::Signal, WeakRef};
    use gtk::CompositeTemplate;
    use once_cell::sync::Lazy;

    #[derive(Debug, Default, CompositeTemplate)]
    #[template(resource = "/io/github/seadve/Mousai/ui/recognized-page.ui")]
    pub struct RecognizedPage {
        #[template_child]
        pub(super) title: TemplateChild<gtk::Label>,
        #[template_child]
        pub(super) subtitle: TemplateChild<gtk::Label>,
        #[template_child]
        pub(super) carousel: TemplateChild<adw::Carousel>,

        pub(super) adaptive_mode: Cell<AdaptiveMode>,
        pub(super) tiles: RefCell<Vec<RecognizedPageTile>>,
        pub(super) player: OnceCell<WeakRef<Player>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for RecognizedPage {
        const NAME: &'static str = "MsaiRecognizedPage";
        type Type = super::RecognizedPage;
        type ParentType = gtk::Widget;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for RecognizedPage {
        fn properties() -> &'static [glib::ParamSpec] {
            static PROPERTIES: Lazy<Vec<glib::ParamSpec>> = Lazy::new(|| {
                vec![
                    glib::ParamSpecEnum::builder::<AdaptiveMode>("adaptive-mode")
                        .explicit_notify()
                        .build(),
                ]
            });

            PROPERTIES.as_ref()
        }

        fn set_property(&self, _id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
            let obj = self.obj();

            match pspec.name() {
                "adaptive-mode" => {
                    let adaptive_mode = value.get().unwrap();
                    obj.set_adaptive_mode(adaptive_mode);
                }
                _ => unimplemented!(),
            }
        }

        fn property(&self, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
            let obj = self.obj();

            match pspec.name() {
                "adaptive-mode" => obj.adaptive_mode().to_value(),
                _ => unimplemented!(),
            }
        }

        fn signals() -> &'static [Signal] {
            static SIGNALS: Lazy<Vec<Signal>> = Lazy::new(|| {
                vec![Signal::builder("song-activated")
                    .param_types([Song::static_type()])
                    .build()]
            });

            SIGNALS.as_ref()
        }

        fn constructed(&self) {
            self.parent_constructed();

            self.obj().update_carousel_spacing();
        }

        fn dispose(&self) {
            self.dispose_template();

            self.obj().unbind_player();
        }
    }

    impl WidgetImpl for RecognizedPage {}
}

glib::wrapper! {
     pub struct RecognizedPage(ObjectSubclass<imp::RecognizedPage>)
        @extends gtk::Widget;
}

impl RecognizedPage {
    pub fn new() -> Self {
        glib::Object::new()
    }

    pub fn set_adaptive_mode(&self, adaptive_mode: AdaptiveMode) {
        if self.adaptive_mode() == adaptive_mode {
            return;
        }

        self.imp().adaptive_mode.set(adaptive_mode);
        self.update_carousel_spacing();
        self.notify("adaptive-mode");
    }

    pub fn adaptive_mode(&self) -> AdaptiveMode {
        self.imp().adaptive_mode.get()
    }

    pub fn connect_song_activated<F>(&self, f: F) -> glib::SignalHandlerId
    where
        F: Fn(&Self, &Song) + 'static,
    {
        self.connect_closure(
            "song-activated",
            true,
            closure_local!(|obj: &Self, song: &Song| {
                f(obj, song);
            }),
        )
    }

    /// Must only be called once
    pub fn bind_player(&self, player: &Player) {
        self.imp().player.set(player.downgrade()).unwrap();
    }

    pub fn unbind_player(&self) {
        for tile in self.imp().tiles.borrow().iter() {
            tile.unbind_player();
        }
    }

    pub fn bind_songs(&self, songs: &[Song]) {
        if songs.is_empty() {
            tracing::warn!("Tried to bound empty song list");
        }

        let imp = self.imp();

        let songs_len = songs.len();
        imp.title.set_label(&ngettext!(
            "Recognized {} new song",
            "Recognized {} new songs",
            songs_len as u32,
            songs_len
        ));
        imp.subtitle.set_label(&ngettext(
            "This song was heard while you're offline",
            "These songs were heard while you're offline",
            songs_len as u32,
        ));

        let player = self
            .imp()
            .player
            .get()
            .expect("Player was not bound")
            .upgrade()
            .expect("Player was dropped");

        for song in songs {
            let tile = RecognizedPageTile::new(song);
            tile.bind_player(&player);
            tile.connect_activated(clone!(@weak self as obj => move |tile| {
                obj.emit_by_name::<()>("song-activated", &[&tile.song()]);
            }));

            imp.carousel.append(&tile);
            imp.tiles.borrow_mut().push(tile);
        }
    }

    fn update_carousel_spacing(&self) {
        let imp = self.imp();

        let spacing = match self.adaptive_mode() {
            AdaptiveMode::Normal => 48,
            AdaptiveMode::Narrow => 6,
        };
        imp.carousel.set_spacing(spacing);
    }
}

impl Default for RecognizedPage {
    fn default() -> Self {
        Self::new()
    }
}
