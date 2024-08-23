use adw::{prelude::*, subclass::prelude::*};
use gtk::glib;

use crate::song::Song;

mod imp {
    use std::cell::RefCell;

    use super::*;

    #[derive(Default, glib::Properties, gtk::CompositeTemplate)]
    #[properties(wrapper_type = super::LyricsPage)]
    #[template(resource = "/io/github/seadve/Mousai/ui/lyrics_page.ui")]
    pub struct LyricsPage {
        #[property(get, set = Self::set_song, explicit_notify, nullable)]
        pub(super) song: RefCell<Option<Song>>,

        #[template_child]
        pub(super) title: TemplateChild<adw::WindowTitle>,
        #[template_child]
        pub(super) label: TemplateChild<gtk::Label>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for LyricsPage {
        const NAME: &'static str = "MousaiLyricsPage";
        type Type = super::LyricsPage;
        type ParentType = adw::NavigationPage;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    #[glib::derived_properties]
    impl ObjectImpl for LyricsPage {}

    impl WidgetImpl for LyricsPage {}
    impl NavigationPageImpl for LyricsPage {}

    impl LyricsPage {
        fn set_song(&self, song: Option<Song>) {
            let obj = self.obj();

            if obj.song() == song {
                return;
            }

            if let Some(song) = &song {
                let title_text = song.artist_title_text();

                obj.set_title(&format!("{} (Lyrics)", title_text));

                self.title.set_title(&title_text);
                self.label.set_text(&song.lyrics().unwrap_or_default());
            } else {
                obj.set_title("Lyrics");

                self.title.set_title("");
                self.label.set_text("");
            }

            self.song.replace(song);
            obj.notify_song();
        }
    }
}

glib::wrapper! {
    pub struct LyricsPage(ObjectSubclass<imp::LyricsPage>)
        @extends gtk::Widget, adw::NavigationPage;
}

impl LyricsPage {
    pub fn new() -> Self {
        glib::Object::new()
    }
}
