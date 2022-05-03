use gtk::{glib, prelude::*, subclass::prelude::*};
use once_cell::sync::Lazy;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use std::{cell::RefCell, rc::Rc};

use super::{external_link::ExternalLink, ExternalLinkList, SongId};
use crate::{
    core::{AlbumArt, DateTime},
    Application,
};

mod imp {
    use super::*;

    #[derive(Debug, Default, Serialize, Deserialize)]
    #[serde(default)]
    pub struct SongInner {
        pub id: SongId,
        pub last_heard: DateTime,
        pub title: String,
        pub artist: String,
        pub album: String,
        pub release_date: String,
        pub external_links: ExternalLinkList,
        pub album_art_link: Option<String>,
        pub playback_link: Option<String>,
        pub lyrics: Option<String>,
    }

    #[derive(Debug, Default)]
    pub struct Song {
        pub inner: RefCell<SongInner>,
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
                        "id",
                        "Id",
                        "Id",
                        SongId::static_type(),
                        glib::ParamFlags::READWRITE | glib::ParamFlags::CONSTRUCT_ONLY,
                    ),
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
                        Some(""),
                        glib::ParamFlags::READWRITE | glib::ParamFlags::CONSTRUCT_ONLY,
                    ),
                    glib::ParamSpecString::new(
                        "artist",
                        "Artist",
                        "Artist of the song",
                        Some(""),
                        glib::ParamFlags::READWRITE | glib::ParamFlags::CONSTRUCT_ONLY,
                    ),
                    glib::ParamSpecString::new(
                        "album",
                        "Album",
                        "Album",
                        Some(""),
                        glib::ParamFlags::READWRITE | glib::ParamFlags::CONSTRUCT_ONLY,
                    ),
                    glib::ParamSpecString::new(
                        "release-date",
                        "Release Date",
                        "Release Date",
                        Some(""),
                        glib::ParamFlags::READWRITE | glib::ParamFlags::CONSTRUCT_ONLY,
                    ),
                    glib::ParamSpecObject::new(
                        "external-links",
                        "External Links",
                        "External Links",
                        ExternalLinkList::static_type(),
                        glib::ParamFlags::READWRITE | glib::ParamFlags::CONSTRUCT_ONLY,
                    ),
                    glib::ParamSpecString::new(
                        "album-art-link",
                        "Album Art Link",
                        "Link where the album art can be downloaded",
                        None,
                        glib::ParamFlags::READWRITE | glib::ParamFlags::CONSTRUCT_ONLY,
                    ),
                    glib::ParamSpecString::new(
                        "playback-link",
                        "Playback Link",
                        "Link containing an excerpt of the song",
                        None,
                        glib::ParamFlags::READWRITE | glib::ParamFlags::CONSTRUCT_ONLY,
                    ),
                    glib::ParamSpecString::new(
                        "lyrics",
                        "Lyrics",
                        "Lyrics",
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
                "id" => {
                    let id = value.get().unwrap();
                    self.inner.borrow_mut().id = id;
                }
                "last-heard" => {
                    let last_heard = value.get().unwrap();
                    obj.set_last_heard(last_heard);
                }
                "title" => {
                    let title = value.get().unwrap();
                    self.inner.borrow_mut().title = title;
                }
                "artist" => {
                    let artist = value.get().unwrap();
                    self.inner.borrow_mut().artist = artist;
                }
                "album" => {
                    let album = value.get().unwrap();
                    self.inner.borrow_mut().album = album;
                }
                "release-date" => {
                    let release_date = value.get().unwrap();
                    self.inner.borrow_mut().release_date = release_date;
                }
                "external-links" => {
                    let external_links = value.get().unwrap();
                    self.inner.borrow_mut().external_links = external_links;
                }
                "album-art-link" => {
                    let album_art_link = value.get().unwrap();
                    self.inner.borrow_mut().album_art_link = album_art_link;
                }
                "playback-link" => {
                    let playback_link = value.get().unwrap();
                    self.inner.borrow_mut().playback_link = playback_link;
                }
                "lyrics" => {
                    let lyrics = value.get().unwrap();
                    obj.set_lyrics(lyrics);
                }
                _ => unimplemented!(),
            }
        }

        fn property(&self, obj: &Self::Type, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
            match pspec.name() {
                "id" => obj.id().to_value(),
                "last-heard" => obj.last_heard().to_value(),
                "title" => obj.title().to_value(),
                "artist" => obj.artist().to_value(),
                "album" => obj.album().to_value(),
                "release-date" => obj.release_date().to_value(),
                "external-links" => obj.external_links().to_value(),
                "album-art-link" => obj.album_art_link().to_value(),
                "playback-link" => obj.playback_link().to_value(),
                "lyrics" => obj.lyrics().to_value(),
                _ => unimplemented!(),
            }
        }
    }
}

glib::wrapper! {
    pub struct Song(ObjectSubclass<imp::Song>);
}

impl Song {
    /// The parameter `SongID` must be unique to each [`Song`] so that [`crate::model::SongList`] will
    /// treat them different.
    ///
    /// The last heard will be the `DateTime` when this is constructed
    pub fn builder(
        id: &SongId,
        title: &str,
        artist: &str,
        album: &str,
        release_date: &str,
    ) -> SongBuilder {
        SongBuilder::new(id, title, artist, album, release_date)
    }

    pub fn id(&self) -> SongId {
        let id = self.imp().inner.borrow().id.clone();

        if id.is_default() {
            log::warn!("SongId was found in default. It should have been set on the construct.");
        }

        id
    }

    pub fn set_last_heard(&self, last_heard: DateTime) {
        self.imp().inner.borrow_mut().last_heard = last_heard;
        self.notify("last-heard");
    }

    pub fn last_heard(&self) -> DateTime {
        self.imp().inner.borrow().last_heard.clone()
    }

    pub fn title(&self) -> String {
        self.imp().inner.borrow().title.clone()
    }

    pub fn artist(&self) -> String {
        self.imp().inner.borrow().artist.clone()
    }

    pub fn album(&self) -> String {
        self.imp().inner.borrow().album.clone()
    }

    pub fn release_date(&self) -> String {
        self.imp().inner.borrow().release_date.clone()
    }

    pub fn external_links(&self) -> ExternalLinkList {
        self.imp().inner.borrow().external_links.clone()
    }

    pub fn album_art_link(&self) -> Option<String> {
        self.imp().inner.borrow().album_art_link.clone()
    }

    pub fn playback_link(&self) -> Option<String> {
        self.imp().inner.borrow().playback_link.clone()
    }

    pub fn set_lyrics(&self, lyrics: Option<&str>) {
        self.imp().inner.borrow_mut().lyrics = lyrics.map(|lyrics| lyrics.to_string());
        self.notify("lyrics");
    }

    pub fn lyrics(&self) -> Option<String> {
        self.imp().inner.borrow().lyrics.clone()
    }

    pub fn album_art(&self) -> anyhow::Result<Rc<AlbumArt>> {
        let album_art_link = self
            .album_art_link()
            .ok_or_else(|| anyhow::anyhow!("Song doesn't have an album art link"))?;

        Ok(Application::default()
            .album_art_store()?
            .get_or_init(&album_art_link))
    }
}

impl Serialize for Song {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.imp().inner.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Song {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let deserialized_inner = imp::SongInner::deserialize(deserializer)?;
        let song: Self = glib::Object::new(&[
            ("id", &deserialized_inner.id),
            ("external-links", &deserialized_inner.external_links),
        ])
        .expect("Failed to create song.");
        song.imp().inner.replace(deserialized_inner);
        Ok(song)
    }
}

pub struct SongBuilder {
    properties: Vec<(&'static str, glib::Value)>,
    external_links: Vec<Box<dyn ExternalLink>>,
}

impl SongBuilder {
    pub fn new(id: &SongId, title: &str, artist: &str, album: &str, release_date: &str) -> Self {
        Self {
            properties: vec![
                ("id", id.to_value()),
                ("title", title.to_value()),
                ("artist", artist.to_value()),
                ("album", album.to_value()),
                ("release-date", release_date.to_value()),
            ],
            external_links: Vec::new(),
        }
    }

    pub fn album_art_link(&mut self, album_art_link: &str) -> &mut Self {
        self.properties
            .push(("album-art-link", album_art_link.to_value()));
        self
    }

    pub fn playback_link(&mut self, playback_link: &str) -> &mut Self {
        self.properties
            .push(("playback-link", playback_link.to_value()));
        self
    }

    pub fn lyrics(&mut self, lyrics: &str) -> &mut Self {
        self.properties.push(("lyrics", lyrics.to_value()));
        self
    }

    pub fn external_link(&mut self, external_link: impl ExternalLink + 'static) -> &mut Self {
        self.external_links.push(Box::new(external_link));
        self
    }

    pub fn build(&mut self) -> Song {
        self.properties.push((
            "external-links",
            ExternalLinkList::new(std::mem::take(&mut self.external_links)).to_value(),
        ));
        glib::Object::with_values(Song::static_type(), &self.properties)
            .expect("Failed to create Song.")
            .downcast()
            .unwrap()
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn properties() {
        let song = Song::builder(
            &SongId::from("UniqueSongId"),
            "Some song",
            "Someone",
            "SomeAlbum",
            "00-00-0000",
        )
        .album_art_link("https://album.png")
        .playback_link("https://test.mp3")
        .lyrics("Some song lyrics")
        .build();

        assert_eq!(song.title(), "Some song");
        assert_eq!(song.artist(), "Someone");
        assert_eq!(song.album(), "SomeAlbum");
        assert_eq!(song.release_date(), "00-00-0000");
        assert_eq!(song.album_art_link().as_deref(), Some("https://album.png"));
        assert_eq!(song.playback_link().as_deref(), Some("https://test.mp3"));
        assert_eq!(song.lyrics().as_deref(), Some("Some song lyrics"));
    }
}
