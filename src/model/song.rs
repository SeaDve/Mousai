use gtk::{glib, prelude::*, subclass::prelude::*};
use once_cell::sync::Lazy;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

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
        pub release_date: Option<String>,
        pub external_links: ExternalLinkList,
        pub album_art_link: Option<String>,
        pub playback_link: Option<String>,
        pub lyrics: Option<String>,
    }

    #[derive(Debug, Default)]
    pub struct Song {
        pub inner: RefCell<SongInner>,
        pub is_newly_recognized: Cell<bool>,
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
                    // Unique ID
                    glib::ParamSpecBoxed::builder("id", SongId::static_type())
                        .flags(glib::ParamFlags::READWRITE | glib::ParamFlags::CONSTRUCT_ONLY)
                        .build(),
                    // DateTime when last heard
                    glib::ParamSpecBoxed::builder("last-heard", DateTime::static_type())
                        .flags(glib::ParamFlags::READWRITE | glib::ParamFlags::EXPLICIT_NOTIFY)
                        .build(),
                    // Title of the song
                    glib::ParamSpecString::builder("title")
                        .flags(glib::ParamFlags::READWRITE | glib::ParamFlags::CONSTRUCT_ONLY)
                        .build(),
                    // Artist of the song
                    glib::ParamSpecString::builder("artist")
                        .flags(glib::ParamFlags::READWRITE | glib::ParamFlags::CONSTRUCT_ONLY)
                        .build(),
                    // Album where the song was from
                    glib::ParamSpecString::builder("album")
                        .flags(glib::ParamFlags::READWRITE | glib::ParamFlags::CONSTRUCT_ONLY)
                        .build(),
                    // Arbitrary string for release date
                    glib::ParamSpecString::builder("release-date")
                        .flags(glib::ParamFlags::READWRITE | glib::ParamFlags::CONSTRUCT_ONLY)
                        .build(),
                    // Links relevant to the song
                    glib::ParamSpecObject::builder(
                        "external-links",
                        ExternalLinkList::static_type(),
                    )
                    .flags(glib::ParamFlags::READWRITE | glib::ParamFlags::CONSTRUCT_ONLY)
                    .build(),
                    // Link where the album art can be downloaded
                    glib::ParamSpecString::builder("album-art-link")
                        .flags(glib::ParamFlags::READWRITE | glib::ParamFlags::CONSTRUCT_ONLY)
                        .build(),
                    // Link containing an excerpt of the song
                    glib::ParamSpecString::builder("playback-link")
                        .flags(glib::ParamFlags::READWRITE | glib::ParamFlags::CONSTRUCT_ONLY)
                        .build(),
                    // Lyrics of the song
                    glib::ParamSpecString::builder("lyrics")
                        .flags(glib::ParamFlags::READWRITE | glib::ParamFlags::EXPLICIT_NOTIFY)
                        .build(),
                    // Whether the song is recently recognized
                    glib::ParamSpecBoolean::builder("is-newly-recognized")
                        .flags(glib::ParamFlags::READWRITE | glib::ParamFlags::EXPLICIT_NOTIFY)
                        .build(),
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
                "is-newly-recognized" => {
                    let is_newly_recognized = value.get().unwrap();
                    obj.set_is_newly_recognized(is_newly_recognized);
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
                "is-newly-recognized" => obj.is_newly_recognized().to_value(),
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
    pub fn builder(id: &SongId, title: &str, artist: &str, album: &str) -> SongBuilder {
        SongBuilder::new(id, title, artist, album)
    }

    pub fn id(&self) -> SongId {
        let id = self.imp().inner.borrow().id.clone();

        if id.is_default() {
            log::warn!("SongId was found in default. It should have been set on the construct.");
        }

        id
    }

    pub fn set_last_heard(&self, value: DateTime) {
        self.imp().inner.borrow_mut().last_heard = value;
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

    pub fn release_date(&self) -> Option<String> {
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

    pub fn set_lyrics(&self, value: Option<&str>) {
        self.imp().inner.borrow_mut().lyrics = value.map(|lyrics| lyrics.to_string());
        self.notify("lyrics");
    }

    pub fn lyrics(&self) -> Option<String> {
        self.imp().inner.borrow().lyrics.clone()
    }

    pub fn is_newly_recognized(&self) -> bool {
        self.imp().is_newly_recognized.get()
    }

    pub fn set_is_newly_recognized(&self, value: bool) {
        if value == self.is_newly_recognized() {
            return;
        }

        self.imp().is_newly_recognized.set(value);
        self.notify("is-newly-recognized");
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
            ("last-heard", &deserialized_inner.last_heard),
            ("title", &deserialized_inner.title),
            ("artist", &deserialized_inner.artist),
            ("album", &deserialized_inner.album),
            ("release-date", &deserialized_inner.release_date),
            ("external-links", &deserialized_inner.external_links),
            ("album-art-link", &deserialized_inner.album_art_link),
            ("playback-link", &deserialized_inner.playback_link),
            ("lyrics", &deserialized_inner.lyrics),
        ])
        .expect("Failed to create song.");
        Ok(song)
    }
}

pub struct SongBuilder {
    properties: Vec<(&'static str, glib::Value)>,
    external_links: Vec<Box<dyn ExternalLink>>,
}

impl SongBuilder {
    pub fn new(id: &SongId, title: &str, artist: &str, album: &str) -> Self {
        Self {
            properties: vec![
                ("id", id.to_value()),
                ("title", title.to_value()),
                ("artist", artist.to_value()),
                ("album", album.to_value()),
            ],
            external_links: Vec::new(),
        }
    }

    pub fn newly_recognized(&mut self, value: bool) -> &mut Self {
        self.properties
            .push(("is-newly-recognized", value.to_value()));
        self
    }

    pub fn release_date(&mut self, value: &str) -> &mut Self {
        self.properties.push(("release-date", value.to_value()));
        self
    }

    pub fn album_art_link(&mut self, value: &str) -> &mut Self {
        self.properties.push(("album-art-link", value.to_value()));
        self
    }

    pub fn playback_link(&mut self, value: &str) -> &mut Self {
        self.properties.push(("playback-link", value.to_value()));
        self
    }

    pub fn lyrics(&mut self, value: &str) -> &mut Self {
        self.properties.push(("lyrics", value.to_value()));
        self
    }

    pub fn external_link(&mut self, value: impl ExternalLink + 'static) -> &mut Self {
        self.external_links.push(Box::new(value));
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
        )
        .release_date("00-00-0000")
        .album_art_link("https://album.png")
        .playback_link("https://test.mp3")
        .lyrics("Some song lyrics")
        .newly_recognized(true)
        .build();

        assert_eq!(song.title(), "Some song");
        assert_eq!(song.artist(), "Someone");
        assert_eq!(song.album(), "SomeAlbum");
        assert_eq!(song.release_date().as_deref(), Some("00-00-0000"));
        assert_eq!(song.album_art_link().as_deref(), Some("https://album.png"));
        assert_eq!(song.playback_link().as_deref(), Some("https://test.mp3"));
        assert_eq!(song.lyrics().as_deref(), Some("Some song lyrics"));
        assert!(song.is_newly_recognized());
    }

    #[test]
    fn deserialize() {
        let song: Song = serde_json::from_str(
            r#"{
                "id": "UniqueSongId",
                "last_heard": "2022-05-14T10:15:37.798479+08",
                "title": "Some song",
                "artist": "Someone",
                "album": "SomeAlbum",
                "release_date": "00-00-0000",
                "external_links": [
                    {
                        "AudDExternalLink": {
                            "uri": "https://aud_d.link"
                        }
                    },
                    {
                        "YoutubeExternalLink": {
                            "search_term": "Someone - Some song"
                        }
                    },
                    {
                        "SpotifyExternalLink": {
                            "uri": "https://spotify.link"
                        }
                    },
                    {
                        "AppleMusicExternalLink": {
                            "uri": "https://apple_music.link"
                        }
                    }
                ],
                "album_art_link": "https://album.png",
                "playback_link": "https://test.mp3",
                "lyrics": "Some song lyrics"
            }"#,
        )
        .expect("Failed to deserialize song.");

        assert_eq!(song.id(), SongId::from("UniqueSongId"));
        assert_eq!(
            song.last_heard().to_iso8601(),
            "2022-05-14T10:15:37.798479+08"
        );
        assert_eq!(song.title(), "Some song");
        assert_eq!(song.artist(), "Someone");
        assert_eq!(song.album(), "SomeAlbum");
        assert_eq!(song.release_date().as_deref(), Some("00-00-0000"));

        assert_eq!(song.external_links().n_items(), 4);
        assert_eq!(
            song.external_links().get(0).unwrap().inner().uri(),
            "https://aud_d.link"
        );
        assert_eq!(
            song.external_links().get(2).unwrap().inner().uri(),
            "https://spotify.link"
        );
        assert_eq!(
            song.external_links().get(3).unwrap().inner().uri(),
            "https://apple_music.link"
        );

        assert_eq!(song.album_art_link().as_deref(), Some("https://album.png"));
        assert_eq!(song.playback_link().as_deref(), Some("https://test.mp3"));
        assert_eq!(song.lyrics().as_deref(), Some("Some song lyrics"));

        assert!(!song.is_newly_recognized());
    }
}
