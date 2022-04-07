use gtk::{glib, prelude::*, subclass::prelude::*};
use once_cell::sync::Lazy;
use once_cell::unsync::OnceCell;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use std::cell::RefCell;

use super::SongId;
use crate::{album_art::AlbumArt, core::DateTime};

mod imp {
    use super::*;

    #[derive(Debug, Default, Serialize, Deserialize)]
    #[serde(default)]
    pub struct SongInner {
        pub last_heard: DateTime,
        pub title: String,
        pub artist: String,
        pub info_link: String,
        pub album_art_link: Option<String>,
        pub playback_link: Option<String>,
    }

    #[derive(Debug, Default)]
    pub struct Song {
        pub inner: RefCell<SongInner>,
        pub album_art: OnceCell<AlbumArt>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Song {
        const NAME: &'static str = "MsaiSong";
        type Type = super::Song;
    }

    impl ObjectImpl for Song {
        fn properties() -> &'static [glib::ParamSpec] {
            static PROPERTIES: Lazy<Vec<glib::ParamSpec>> = Lazy::new(|| {
                vec![
                    glib::ParamSpecBoxed::new(
                        "last-heard",
                        "Last Heard",
                        "The DateTime when this was last heard",
                        DateTime::static_type(),
                        glib::ParamFlags::READWRITE | glib::ParamFlags::EXPLICIT_NOTIFY,
                    ),
                    glib::ParamSpecString::new(
                        "title",
                        "Title",
                        "Title of the song",
                        None,
                        glib::ParamFlags::READWRITE | glib::ParamFlags::EXPLICIT_NOTIFY,
                    ),
                    glib::ParamSpecString::new(
                        "artist",
                        "Artist",
                        "Artist of the song",
                        None,
                        glib::ParamFlags::READWRITE | glib::ParamFlags::EXPLICIT_NOTIFY,
                    ),
                    glib::ParamSpecString::new(
                        "info-link",
                        "Info Link",
                        "Link to website containing song information",
                        None,
                        glib::ParamFlags::READWRITE | glib::ParamFlags::EXPLICIT_NOTIFY,
                    ),
                    glib::ParamSpecString::new(
                        "album-art-link",
                        "Album Art Link",
                        "Link where the album art can be downloaded",
                        None,
                        glib::ParamFlags::READWRITE | glib::ParamFlags::EXPLICIT_NOTIFY,
                    ),
                    glib::ParamSpecString::new(
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
                "last-heard" => {
                    let last_heard = value.get().unwrap();
                    obj.set_last_heard(last_heard);
                }
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
                "album-art-link" => {
                    let album_art_link = value.get().unwrap();
                    obj.set_album_art_link(album_art_link);
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
                "last-heard" => obj.last_heard().to_value(),
                "title" => obj.title().to_value(),
                "artist" => obj.artist().to_value(),
                "info-link" => obj.info_link().to_value(),
                "album-art-link" => obj.album_art_link().to_value(),
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
    /// treat them different.
    ///
    /// The last heard will be the `DateTime` when this is constructed
    pub fn new(title: &str, artist: &str, info_link: &str) -> Self {
        glib::Object::builder()
            .property("last-heard", DateTime::now())
            .property("title", title)
            .property("artist", artist)
            .property("info-link", info_link)
            .build()
            .expect("Failed to create Song.")
    }

    pub fn set_last_heard(&self, last_heard: DateTime) {
        self.imp().inner.borrow_mut().last_heard = last_heard;
        self.notify("last-heard");
    }

    pub fn last_heard(&self) -> DateTime {
        self.imp().inner.borrow().last_heard
    }

    pub fn set_title(&self, title: &str) {
        self.imp().inner.borrow_mut().title = title.to_string();
        self.notify("title");
    }

    pub fn title(&self) -> String {
        self.imp().inner.borrow().title.clone()
    }

    pub fn set_artist(&self, artist: &str) {
        self.imp().inner.borrow_mut().artist = artist.to_string();
        self.notify("artist");
    }

    pub fn artist(&self) -> String {
        self.imp().inner.borrow().artist.clone()
    }

    pub fn set_info_link(&self, info_link: &str) {
        self.imp().inner.borrow_mut().info_link = info_link.to_string();
        self.notify("info-link");
    }

    pub fn info_link(&self) -> String {
        self.imp().inner.borrow().info_link.clone()
    }

    pub fn set_album_art_link(&self, album_art_link: Option<&str>) {
        self.imp().inner.borrow_mut().album_art_link = album_art_link.map(str::to_string);
        self.notify("album-art-link");
    }

    pub fn album_art_link(&self) -> Option<String> {
        self.imp().inner.borrow().album_art_link.clone()
    }

    pub fn set_playback_link(&self, playback_link: Option<&str>) {
        self.imp().inner.borrow_mut().playback_link = playback_link.map(str::to_string);
        self.notify("playback-link");
    }

    pub fn playback_link(&self) -> Option<String> {
        self.imp().inner.borrow().playback_link.clone()
    }

    pub fn id(&self) -> SongId {
        // Song's info_link is unique to every song
        SongId::new(&self.info_link())
    }

    pub fn album_art(&self) -> anyhow::Result<&AlbumArt> {
        self.imp()
            .album_art
            .get_or_try_init(|| AlbumArt::for_song(self))
    }
}

impl Serialize for Song {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.imp().inner.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Song {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let song_inner = imp::SongInner::deserialize(deserializer)?;

        let song: Song = glib::Object::new(&[]).expect("Failed to create Song.");
        song.imp().inner.replace(song_inner);

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
