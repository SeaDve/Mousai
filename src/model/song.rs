use anyhow::{anyhow, Result};
use gtk::{glib, prelude::*, subclass::prelude::*};
use once_cell::unsync::OnceCell;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

use super::{external_link::ExternalLink, ExternalLinkList, SongId};
use crate::{
    core::{AlbumArt, DateTime},
    serde::{deserialize_once_cell, serialize_once_cell},
    utils,
};

mod imp {
    use super::*;

    #[derive(Debug, Default, glib::Properties, Serialize, Deserialize)]
    #[properties(wrapper_type = super::Song)]
    #[serde(default)]
    pub struct Song {
        /// Unique ID
        #[property(get, set, construct_only)]
        #[serde(
            serialize_with = "serialize_once_cell",
            deserialize_with = "deserialize_once_cell"
        )]
        pub(super) id: OnceCell<SongId>,
        /// Title of the song
        #[property(get, set, construct_only)]
        pub(super) title: RefCell<String>,
        /// Artist of the song
        #[property(get, set, construct_only)]
        pub(super) artist: RefCell<String>,
        /// Album where the song was from
        #[property(get, set, construct_only)]
        pub(super) album: RefCell<String>,
        /// Arbitrary string for release date
        #[property(get, set, construct_only)]
        pub(super) release_date: RefCell<Option<String>>,
        /// Links relevant to the song
        #[property(get, set, construct_only)]
        pub(super) external_links: RefCell<ExternalLinkList>,
        /// Link where the album art can be downloaded
        #[property(get, set, construct_only)]
        pub(super) album_art_link: RefCell<Option<String>>,
        /// Link to a sample of the song
        #[property(get, set, construct_only)]
        pub(super) playback_link: RefCell<Option<String>>,
        /// Lyrics of the song
        #[property(get, set, construct_only)]
        pub(super) lyrics: RefCell<Option<String>>,
        /// Date and time when last heard
        #[property(get, set = Self::set_last_heard, explicit_notify)]
        pub(super) last_heard: RefCell<DateTime>,
        /// Whether the song was heard for the first time
        #[property(get, set = Self::set_is_newly_heard, explicit_notify)]
        pub(super) is_newly_heard: Cell<bool>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Song {
        const NAME: &'static str = "MsaiSong";
        type Type = super::Song;
    }

    impl ObjectImpl for Song {
        crate::derived_properties!();
    }

    impl Song {
        fn set_last_heard(&self, value: DateTime) {
            let obj = self.obj();

            self.last_heard.replace(value);
            obj.notify_last_heard();
        }

        fn set_is_newly_heard(&self, value: bool) {
            let obj = self.obj();

            if value == obj.is_newly_heard() {
                return;
            }

            self.is_newly_heard.set(value);
            obj.notify_is_newly_heard();
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

    /// String to match to when searching for self.
    pub fn search_term(&self) -> String {
        format!("{}{}", self.title(), self.artist())
    }

    /// String copied to clipboard when copying self.
    pub fn copy_term(&self) -> String {
        format!("{} - {}", self.artist(), self.title())
    }

    pub fn album_art(&self) -> Result<Rc<AlbumArt>> {
        let album_art_link = self
            .album_art_link()
            .ok_or_else(|| anyhow!("Song doesn't have an album art link"))?;

        utils::app_instance()
            .album_art_store()?
            .get_or_init(&album_art_link)
    }
}

impl Serialize for Song {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.imp().serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Song {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let deserialized_imp = imp::Song::deserialize(deserializer)?;
        Ok(glib::Object::builder()
            .property("id", deserialized_imp.id.into_inner().unwrap_or_default())
            .property("title", deserialized_imp.title.take())
            .property("artist", deserialized_imp.artist.take())
            .property("album", deserialized_imp.album.take())
            .property("release-date", deserialized_imp.release_date.take())
            .property("external-links", deserialized_imp.external_links.take())
            .property("album-art-link", deserialized_imp.album_art_link.take())
            .property("playback-link", deserialized_imp.playback_link.take())
            .property("lyrics", deserialized_imp.lyrics.take())
            .property("last-heard", deserialized_imp.last_heard.take())
            .property("is-newly-heard", deserialized_imp.is_newly_heard.take())
            .build())
    }
}

#[must_use = "builder doesn't do anything unless built"]
pub struct SongBuilder {
    properties: Vec<(&'static str, glib::Value)>,
    external_links: ExternalLinkList,
}

impl SongBuilder {
    pub fn new(id: &SongId, title: &str, artist: &str, album: &str) -> Self {
        Self {
            properties: vec![
                ("id", id.into()),
                ("title", title.into()),
                ("artist", artist.into()),
                ("album", album.into()),
            ],
            external_links: ExternalLinkList::default(),
        }
    }

    pub fn newly_heard(&mut self, value: bool) -> &mut Self {
        self.properties.push(("is-newly-heard", value.into()));
        self
    }

    pub fn release_date(&mut self, value: &str) -> &mut Self {
        self.properties.push(("release-date", value.into()));
        self
    }

    pub fn album_art_link(&mut self, value: &str) -> &mut Self {
        self.properties.push(("album-art-link", value.into()));
        self
    }

    pub fn playback_link(&mut self, value: &str) -> &mut Self {
        self.properties.push(("playback-link", value.into()));
        self
    }

    pub fn lyrics(&mut self, value: &str) -> &mut Self {
        self.properties.push(("lyrics", value.into()));
        self
    }

    /// Pushes an external link. This is not idempotent.
    pub fn external_link(&mut self, value: impl ExternalLink + 'static) -> &mut Self {
        self.external_links.push(value);
        self
    }

    pub fn build(&mut self) -> Song {
        self.properties
            .push(("external-links", self.external_links.to_value()));
        glib::Object::with_mut_values(Song::static_type(), &mut self.properties)
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
            &SongId::new("UniqueSongId"),
            "Some song",
            "Someone",
            "SomeAlbum",
        )
        .release_date("00-00-0000")
        .album_art_link("https://album.png")
        .playback_link("https://test.mp3")
        .lyrics("Some song lyrics")
        .newly_heard(true)
        .build();

        assert_eq!(song.title(), "Some song");
        assert_eq!(song.artist(), "Someone");
        assert_eq!(song.album(), "SomeAlbum");
        assert_eq!(song.release_date().as_deref(), Some("00-00-0000"));
        assert_eq!(song.album_art_link().as_deref(), Some("https://album.png"));
        assert_eq!(song.playback_link().as_deref(), Some("https://test.mp3"));
        assert_eq!(song.lyrics().as_deref(), Some("Some song lyrics"));
        assert!(song.is_newly_heard());
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
        .unwrap();

        assert_eq!(song.id(), SongId::new("UniqueSongId"));
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

        assert!(!song.is_newly_heard());
    }

    #[test]
    fn deserialize_without_song_id() {
        let song_1: Song = serde_json::from_str("{}").unwrap();
        let song_2: Song = serde_json::from_str("{}").unwrap();
        let song_3: Song = serde_json::from_str("{}").unwrap();

        // Make sure that the song id is unique
        // even it is not defined
        assert_ne!(song_1.id(), song_2.id());
        assert_ne!(song_2.id(), song_3.id());
    }
}
