use gtk::{glib, prelude::*, subclass::prelude::*};

use std::cell::RefCell;

use super::SongId;

mod imp {
    use super::*;
    use once_cell::sync::Lazy;

    #[derive(Debug, Default)]
    pub struct Song {
        pub title: RefCell<String>,
        pub artist: RefCell<String>,
        pub info_link: RefCell<String>,
        pub playback_link: RefCell<Option<String>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Song {
        const NAME: &'static str = "MsaiSong";
        type Type = super::Song;
        type ParentType = glib::Object;
    }

    impl ObjectImpl for Song {
        fn properties() -> &'static [glib::ParamSpec] {
            static PROPERTIES: Lazy<Vec<glib::ParamSpec>> = Lazy::new(|| {
                vec![
                    glib::ParamSpec::new_string(
                        "title",
                        "Title",
                        "Title of the song",
                        None,
                        glib::ParamFlags::READWRITE | glib::ParamFlags::EXPLICIT_NOTIFY,
                    ),
                    glib::ParamSpec::new_string(
                        "artist",
                        "Artish",
                        "Artist of the song",
                        None,
                        glib::ParamFlags::READWRITE | glib::ParamFlags::EXPLICIT_NOTIFY,
                    ),
                    glib::ParamSpec::new_string(
                        "info-link",
                        "Info Link",
                        "Link to website containing song information",
                        None,
                        glib::ParamFlags::READWRITE | glib::ParamFlags::EXPLICIT_NOTIFY,
                    ),
                    glib::ParamSpec::new_string(
                        "playback-link",
                        "Playback Link",
                        "Link containing an excerpt of the song",
                        None,
                        glib::ParamFlags::READWRITE | glib::ParamFlags::EXPLICIT_NOTIFY,
                    ),
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
                "title" => {
                    let title = value.get().unwrap();
                    obj.set_title(title);
                }
                "artist" => {
                    let artist = value.get().unwrap();
                    obj.set_artist(artist);
                }
                "info-link" => {
                    let info_link = value.get().unwrap();
                    obj.set_info_link(info_link);
                }
                "playback-link" => {
                    let playback_link = value.get().unwrap();
                    obj.set_playback_link(playback_link);
                }
                _ => unimplemented!(),
            }
        }

        fn property(&self, obj: &Self::Type, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
            match pspec.name() {
                "title" => obj.title().to_value(),
                "artist" => obj.artist().to_value(),
                "info-link" => obj.info_link().to_value(),
                "playback-link" => obj.playback_link().to_value(),
                _ => unimplemented!(),
            }
        }
    }
}

glib::wrapper! {
    pub struct Song(ObjectSubclass<imp::Song>);
}

impl Song {
    pub fn new(title: &str, artist: &str, info_link: &str) -> Self {
        glib::Object::new(&[
            ("title", &title.to_string()),
            ("artist", &artist.to_string()),
            ("info-link", &info_link.to_string()),
        ])
        .expect("Failed to create Song.")
    }

    pub fn set_title(&self, title: &str) {
        let imp = imp::Song::from_instance(self);
        imp.title.replace(title.to_string());
        self.notify("title");
    }

    pub fn title(&self) -> String {
        let imp = imp::Song::from_instance(self);
        imp.title.borrow().clone()
    }

    pub fn set_artist(&self, artist: &str) {
        let imp = imp::Song::from_instance(self);
        imp.artist.replace(artist.to_string());
        self.notify("artist");
    }

    pub fn artist(&self) -> String {
        let imp = imp::Song::from_instance(self);
        imp.artist.borrow().clone()
    }

    pub fn set_info_link(&self, info_link: &str) {
        let imp = imp::Song::from_instance(self);
        imp.info_link.replace(info_link.to_string());
        self.notify("info-link");
    }

    pub fn info_link(&self) -> String {
        let imp = imp::Song::from_instance(self);
        imp.info_link.borrow().clone()
    }

    pub fn set_playback_link(&self, playback_link: Option<&str>) {
        let imp = imp::Song::from_instance(self);
        imp.playback_link
            .replace(playback_link.map(|pl| pl.to_string()));
        self.notify("playback-link");
    }

    pub fn playback_link(&self) -> Option<String> {
        let imp = imp::Song::from_instance(self);
        imp.playback_link.borrow().clone()
    }

    pub fn id(&self) -> SongId {
        // Song's info_link is unique to every song
        SongId::new(&self.info_link())
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn properties() {
        let song = Song::new("Some song", "Someone", "https://somewhere.com");
        assert_eq!(song.title(), "Some song");
        assert_eq!(song.artist(), "Someone");
        assert_eq!(song.info_link(), "https://somewhere.com");
        assert_eq!(song.playback_link(), None);

        song.set_title("New title");
        assert_eq!(song.title(), "New title");

        song.set_artist("New artist");
        assert_eq!(song.artist(), "New artist");

        song.set_info_link("New info link");
        assert_eq!(song.info_link(), "New info link");

        song.set_playback_link(Some("https:://playbacklink.somewhere.com"));
        assert_eq!(
            song.playback_link().as_deref(),
            Some("https:://playbacklink.somewhere.com")
        );
    }
}
