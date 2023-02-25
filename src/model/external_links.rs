use gtk::{gio, glib, prelude::*, subclass::prelude::*};
use indexmap::IndexMap;
use rusqlite::types::{FromSql, FromSqlError, FromSqlResult, ToSql, ToSqlOutput, ValueRef};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use std::{cell::RefCell, collections::HashMap};

use super::external_link::ExternalLink;

/// Known keys for external links.
#[derive(Debug, Clone, Copy, strum::EnumString, strum::AsRefStr)]
#[strum(serialize_all = "kebab-case")]
pub enum ExternalLinkKey {
    AppleMusicUrl,
    AudDUrl,
    SpotifyUrl,
    YoutubeSearchTerm,
}

mod imp {
    use super::*;

    #[derive(Debug, Default)]
    pub struct ExternalLinks {
        pub(super) map: RefCell<IndexMap<String, String>>,
        pub(super) cache: RefCell<HashMap<String, ExternalLink>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for ExternalLinks {
        const NAME: &'static str = "MsaiExternalLinks";
        type Type = super::ExternalLinks;
        type Interfaces = (gio::ListModel,);
    }

    impl ObjectImpl for ExternalLinks {}

    impl ListModelImpl for ExternalLinks {
        fn item_type(&self) -> glib::Type {
            ExternalLink::static_type()
        }

        fn n_items(&self) -> u32 {
            self.obj().len() as u32
        }

        fn item(&self, position: u32) -> Option<glib::Object> {
            let map = self.map.borrow();
            let (key, value) = map.get_index(position as usize)?;

            Some(
                self.cache
                    .borrow_mut()
                    .entry(key.to_string())
                    .or_insert_with(|| ExternalLink::new(key.to_string(), value.to_string()))
                    .clone()
                    .upcast::<glib::Object>(),
            )
        }
    }
}

glib::wrapper! {
    pub struct ExternalLinks(ObjectSubclass<imp::ExternalLinks>)
        @implements gio::ListModel;
}

impl ExternalLinks {
    pub fn new() -> Self {
        glib::Object::new()
    }

    pub fn insert(&self, key: ExternalLinkKey, value: String) -> bool {
        let (position, last_value) = self
            .imp()
            .map
            .borrow_mut()
            .insert_full(key.as_ref().to_string(), value);

        // FIXME handle in db

        if last_value.is_some() {
            self.items_changed(position as u32, 1, 1);
            false
        } else {
            self.items_changed(position as u32, 0, 1);
            true
        }
    }

    pub fn get(&self, key: ExternalLinkKey) -> Option<String> {
        self.imp().map.borrow().get(key.as_ref()).cloned()
    }

    pub fn len(&self) -> usize {
        self.imp().map.borrow().len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl Default for ExternalLinks {
    fn default() -> Self {
        Self::new()
    }
}

impl FromSql for ExternalLinks {
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        match serde_json::from_slice::<IndexMap<String, String>>(value.as_bytes()?) {
            Ok(map) => {
                let this = Self::new();
                this.imp().map.replace(map);
                Ok(this)
            }
            Err(err) => Err(FromSqlError::Other(err.into())),
        }
    }
}

impl ToSql for ExternalLinks {
    fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
        match serde_json::to_vec(&self.imp().map) {
            Ok(bytes) => Ok(ToSqlOutput::from(bytes)),
            Err(err) => Err(rusqlite::Error::ToSqlConversionFailure(err.into())),
        }
    }
}

impl Serialize for ExternalLinks {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.imp().map.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for ExternalLinks {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let external_links = IndexMap::<String, String>::deserialize(deserializer)?;

        let this = Self::new();
        this.imp().map.replace(external_links);

        Ok(this)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::{cell::Cell, rc::Rc};

    #[test]
    fn item() {
        let links = ExternalLinks::default();

        links.insert(ExternalLinkKey::YoutubeSearchTerm, "A".to_string());
        links.insert(ExternalLinkKey::SpotifyUrl, "B".to_string());

        let a = links.item(0).unwrap().downcast::<ExternalLink>().unwrap();
        assert_eq!(a.key(), ExternalLinkKey::YoutubeSearchTerm.as_ref());
        assert_eq!(a.value(), "A");

        let b = links.item(1).unwrap().downcast::<ExternalLink>().unwrap();
        assert_eq!(b.key(), ExternalLinkKey::SpotifyUrl.as_ref());
        assert_eq!(b.value(), "B");
    }

    #[test]
    fn item_cache() {
        let links = ExternalLinks::default();

        links.insert(ExternalLinkKey::YoutubeSearchTerm, "A".to_string());

        assert_eq!(links.imp().cache.borrow().len(), 0);

        let a_1 = links.item(0).unwrap().downcast::<ExternalLink>().unwrap();
        assert_eq!(links.imp().cache.borrow().len(), 1);

        let a_2 = links.item(0).unwrap().downcast::<ExternalLink>().unwrap();
        assert_eq!(links.imp().cache.borrow().len(), 1);

        assert_eq!(a_1, a_2);
    }

    #[test]
    fn items_changed_insert() {
        let links = ExternalLinks::default();

        links.connect_items_changed(|_, index, removed, added| {
            assert_eq!(index, 0);
            assert_eq!(removed, 0);
            assert_eq!(added, 1);
        });

        links.insert(ExternalLinkKey::YoutubeSearchTerm, "A".to_string());
    }

    #[test]
    fn items_changed_insert_eq_key() {
        let links = ExternalLinks::default();
        assert!(links.insert(ExternalLinkKey::YoutubeSearchTerm, "A".to_string()));

        let n_called = Rc::new(Cell::new(0));

        let n_called_clone = Rc::clone(&n_called);
        links.connect_items_changed(move |_, index, removed, added| {
            assert_eq!(index, 0);
            assert_eq!(removed, 1);
            assert_eq!(added, 1);
            n_called_clone.set(n_called_clone.get() + 1);
        });

        assert_eq!(n_called.get(), 0);
        assert!(!links.insert(ExternalLinkKey::YoutubeSearchTerm, "B".to_string()));
        assert_eq!(n_called.get(), 1);
    }

    #[test]
    fn deserialize() {
        let links: ExternalLinks = serde_json::from_str(
            r#"{
            "apple-music-url": "https://apple_music.link",
            "aud-d-url": "https://aud_d.link",
            "spotify-url": "https://spotify.link",
            "youtube-search-term": "Someone - Some song",
            "extra": "extra"
            }"#,
        )
        .unwrap();

        assert_eq!(links.len(), 5);
        assert_eq!(
            links.get(ExternalLinkKey::AppleMusicUrl).as_deref(),
            Some("https://apple_music.link")
        );
        assert_eq!(
            links.get(ExternalLinkKey::AudDUrl).as_deref(),
            Some("https://aud_d.link")
        );
        assert_eq!(
            links.get(ExternalLinkKey::SpotifyUrl).as_deref(),
            Some("https://spotify.link")
        );
        assert_eq!(
            links.get(ExternalLinkKey::YoutubeSearchTerm).as_deref(),
            Some("Someone - Some song")
        );
    }

    #[test]
    fn serialize() {
        let links = ExternalLinks::new();
        links.insert(
            ExternalLinkKey::AppleMusicUrl,
            "https://apple_music.link".to_string(),
        );
        links.insert(ExternalLinkKey::AudDUrl, "https://aud_d.link".to_string());
        links.insert(
            ExternalLinkKey::SpotifyUrl,
            "https://spotify.link".to_string(),
        );
        links.insert(
            ExternalLinkKey::YoutubeSearchTerm,
            "Someone - Some song".to_string(),
        );

        let json = serde_json::to_string(&links).unwrap();

        assert_eq!(
            json,
            r#"{"apple-music-url":"https://apple_music.link","aud-d-url":"https://aud_d.link","spotify-url":"https://spotify.link","youtube-search-term":"Someone - Some song"}"#
        );
    }
}
