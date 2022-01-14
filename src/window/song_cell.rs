use gtk::{glib, prelude::*, subclass::prelude::*};

use std::cell::RefCell;

use crate::model::Song;

mod imp {
    use super::*;
    use gtk::CompositeTemplate;
    use once_cell::sync::Lazy;

    #[derive(Debug, Default, CompositeTemplate)]
    #[template(resource = "/io/github/seadve/Mousai/ui/song-cell.ui")]
    pub struct SongCell {
        pub song: RefCell<Option<Song>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for SongCell {
        const NAME: &'static str = "MsaiSongCell";
        type Type = super::SongCell;
        type ParentType = gtk::Widget;

        fn class_init(klass: &mut Self::Class) {
            Self::bind_template(klass);
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for SongCell {
        fn properties() -> &'static [glib::ParamSpec] {
            static PROPERTIES: Lazy<Vec<glib::ParamSpec>> = Lazy::new(|| {
                vec![glib::ParamSpec::new_object(
                    "song",
                    "Song",
                    "Song represented by Self",
                    Song::static_type(),
                    glib::ParamFlags::READWRITE | glib::ParamFlags::EXPLICIT_NOTIFY,
                )]
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
                "song" => {
                    let song = value.get().unwrap();
                    obj.set_song(song);
                }
                _ => unimplemented!(),
            }
        }

        fn property(&self, obj: &Self::Type, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
            match pspec.name() {
                "song" => obj.song().to_value(),
                _ => unimplemented!(),
            }
        }

        fn dispose(&self, obj: &Self::Type) {
            while let Some(child) = obj.first_child() {
                child.unparent();
            }
        }
    }

    impl WidgetImpl for SongCell {}
}

glib::wrapper! {
    pub struct SongCell(ObjectSubclass<imp::SongCell>)
        @extends gtk::Widget;
}

impl SongCell {
    pub fn new() -> Self {
        glib::Object::new(&[]).expect("Failed to create SongCell")
    }

    pub fn set_song(&self, song: Option<Song>) {
        if song == self.song() {
            return;
        }

        let imp = imp::SongCell::from_instance(self);
        imp.song.replace(song);
        self.notify("song");
    }

    pub fn song(&self) -> Option<Song> {
        let imp = imp::SongCell::from_instance(self);
        imp.song.borrow().clone()
    }
}

impl Default for SongCell {
    fn default() -> Self {
        Self::new()
    }
}
