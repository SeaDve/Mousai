use gtk::{glib, prelude::*, subclass::prelude::*};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use std::cell::RefCell;

use super::SongId;

mod imp {
    use super::*;
    use once_cell::sync::Lazy;

    #[derive(Debug, Default, Serialize, Deserialize)]
    pub struct SongInner {
        pub title: String,
        pub artist: String,
        pub info_link: String,
        pub playback_link: Option<String>,
    }

    #[derive(Debug, Default)]
    pub struct Song {
        pub inner: RefCell<SongInner>,
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
    /// The parameter `info_link` must be unique to each [`Song`] so that [`SongList`] will
    /// treat them different
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
        imp.inner.borrow_mut().title = title.to_string();
        self.notify("title");
    }

    pub fn title(&self) -> String {
        let imp = imp::Song::from_instance(self);
        imp.inner.borrow().title.clone()
    }

    pub fn set_artist(&self, artist: &str) {
        let imp = imp::Song::from_instance(self);
        imp.inner.borrow_mut().artist = artist.to_string();
        self.notify("artist");
    }

    pub fn artist(&self) -> String {
        let imp = imp::Song::from_instance(self);
        imp.inner.borrow().artist.clone()
    }

    pub fn set_info_link(&self, info_link: &str) {
        let imp = imp::Song::from_instance(self);
        imp.inner.borrow_mut().info_link = info_link.to_string();
        self.notify("info-link");
    }

    pub fn info_link(&self) -> String {
        let imp = imp::Song::from_instance(self);
        imp.inner.borrow().info_link.clone()
    }

    pub fn set_playback_link(&self, playback_link: Option<&str>) {
        let imp = imp::Song::from_instance(self);
        imp.inner.borrow_mut().playback_link = playback_link.map(str::to_string);
        self.notify("playback-link");
    }

    pub fn playback_link(&self) -> Option<String> {
        let imp = imp::Song::from_instance(self);
        imp.inner.borrow().playback_link.clone()
    }

    pub fn id(&self) -> SongId {
        // Song's info_link is unique to every song
        SongId::new(&self.info_link())
    }
}

impl Serialize for Song {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let imp = imp::Song::from_instance(self);
        imp.inner.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Song {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let song_inner = imp::SongInner::deserialize(deserializer)?;

        let song: Song = glib::Object::new(&[]).expect("Failed to create Song.");
        imp::Song::from_instance(&song).inner.replace(song_inner);

        Ok(song)
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
