use adw::{
    prelude::*,
    subclass::{navigation_page::NavigationPageImpl, prelude::*},
};
use gettextrs::ngettext;
use gtk::glib::{self, clone, closure_local};

use std::cell::{Cell, OnceCell, RefCell};

use super::{recognized_page_tile::RecognizedPageTile, AdaptiveMode};
use crate::{i18n::ngettext_f, player::Player, song::Song};

mod imp {
    use glib::{subclass::Signal, WeakRef};
    use once_cell::sync::Lazy;

    use super::*;

    #[derive(Default, glib::Properties, gtk::CompositeTemplate)]
    #[properties(wrapper_type = super::RecognizedPage)]
    #[template(resource = "/io/github/seadve/Mousai/ui/recognized-page.ui")]
    pub struct RecognizedPage {
        #[property(get, set = Self::set_adaptive_mode, explicit_notify, builder(AdaptiveMode::default()))]
        pub(super) adaptive_mode: Cell<AdaptiveMode>,

        #[template_child]
        pub(super) heading: TemplateChild<gtk::Label>,
        #[template_child]
        pub(super) body: TemplateChild<gtk::Label>,
        #[template_child]
        pub(super) carousel: TemplateChild<adw::Carousel>,

        pub(super) tiles: RefCell<Vec<RecognizedPageTile>>,
        pub(super) player: OnceCell<WeakRef<Player>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for RecognizedPage {
        const NAME: &'static str = "MsaiRecognizedPage";
        type Type = super::RecognizedPage;
        type ParentType = adw::NavigationPage;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    #[glib::derived_properties]
    impl ObjectImpl for RecognizedPage {
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
            self.obj().unbind_player();
        }
    }

    impl WidgetImpl for RecognizedPage {}
    impl NavigationPageImpl for RecognizedPage {}

    impl RecognizedPage {
        fn set_adaptive_mode(&self, adaptive_mode: AdaptiveMode) {
            let obj = self.obj();

            if obj.adaptive_mode() == adaptive_mode {
                return;
            }

            self.adaptive_mode.set(adaptive_mode);
            obj.update_carousel_spacing();
            obj.notify_adaptive_mode();
        }
    }
}

glib::wrapper! {
     pub struct RecognizedPage(ObjectSubclass<imp::RecognizedPage>)
        @extends gtk::Widget, adw::NavigationPage;
}

impl RecognizedPage {
    pub fn new() -> Self {
        glib::Object::new()
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

        let n_songs = songs.len();
        self.set_title(&ngettext(
            "Recognized Song",
            "Recognized Songs",
            n_songs as u32,
        ));
        imp.heading.set_label(&ngettext_f(
            // Translators: Do NOT translate the contents between '{' and '}', this is a variable name.
            "Recognized {n_songs} New Song",
            "Recognized {n_songs} New Songs",
            n_songs as u32,
            &[("n_songs", &n_songs.to_string())],
        ));
        imp.body.set_label(&ngettext(
            "This song was recognized from your saved recording",
            "These songs were recognized from your saved recordings",
            n_songs as u32,
        ));

        let player = self
            .imp()
            .player
            .get()
            .expect("player must be bound")
            .upgrade()
            .expect("player must not be dropped");
        for song in songs {
            let tile = RecognizedPageTile::new(song);
            tile.bind_player(&player);
            tile.connect_activated(clone!(
                #[weak(rename_to = obj)]
                self,
                move |tile| {
                    obj.emit_by_name::<()>("song-activated", &[&tile.song()]);
                }
            ));

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
