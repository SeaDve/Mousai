use anyhow::{Context, Result};
use gtk::{
    gdk, gio,
    glib::{self, clone},
    prelude::*,
    subclass::prelude::*,
};
use once_cell::unsync::OnceCell;
use soup::prelude::*;

use std::{cell::RefCell, marker::PhantomData, path::Path};

use crate::utils;

// TODO
// - Don't load AlbumArt if network is metered
// - Retry downloading once network is back
// - Integrate more with AlbumCover widget
// - Load only at most n AlbumArt at a time
// - Sanitize the arbitrary data downloaded before converting it to texture

mod imp {
    use super::*;

    #[derive(Debug, Default, glib::Properties)]
    #[properties(wrapper_type = super::AlbumArt)]
    pub struct AlbumArt {
        #[property(get, set, construct_only)]
        pub(super) session: OnceCell<soup::Session>,
        #[property(get, set, construct_only)]
        pub(super) download_url: OnceCell<String>,
        #[property(get, set, construct_only)]
        pub(super) cache_file: OnceCell<gio::File>,
        #[property(get = Self::is_loaded)]
        pub(super) is_loaded: PhantomData<bool>,

        pub(super) texture: RefCell<Option<gdk::Texture>>,
        pub(super) join_handle: RefCell<Option<glib::JoinHandle<()>>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for AlbumArt {
        const NAME: &'static str = "MsaiAlbumArt";
        type Type = super::AlbumArt;
        type Interfaces = (gdk::Paintable,);
    }

    impl ObjectImpl for AlbumArt {
        fn constructed(&self) {
            self.parent_constructed();

            let obj = self.obj();

            let join_handle = utils::spawn(
                glib::PRIORITY_LOW,
                clone!(@weak obj => async move {
                    if let Err(err) = obj.load_texture().await {
                        tracing::warn!("Failed to load album art: {:?}", err);
                    }
                }),
            );
            self.join_handle.replace(Some(join_handle));
        }

        fn dispose(&self) {
            if let Some(join_handle) = self.join_handle.take() {
                join_handle.abort();
            }
        }

        crate::derived_properties!();
    }

    impl PaintableImpl for AlbumArt {
        fn snapshot(&self, snapshot: &gdk::Snapshot, width: f64, height: f64) {
            if let Some(texture) = self.texture.borrow().as_ref() {
                texture.snapshot(snapshot, width, height);
            }
        }

        fn current_image(&self) -> gdk::Paintable {
            self.texture.borrow().as_ref().map_or_else(
                || self.parent_current_image(),
                |texture| texture.current_image(),
            )
        }

        fn flags(&self) -> gdk::PaintableFlags {
            self.texture.borrow().as_ref().map_or_else(
                || self.parent_flags(),
                |texture| {
                    let mut flags = texture.flags();
                    flags.remove(gdk::PaintableFlags::CONTENTS);
                    flags
                },
            )
        }

        fn intrinsic_width(&self) -> i32 {
            self.texture.borrow().as_ref().map_or_else(
                || self.parent_intrinsic_width(),
                |texture| texture.intrinsic_width(),
            )
        }

        fn intrinsic_height(&self) -> i32 {
            self.texture.borrow().as_ref().map_or_else(
                || self.parent_intrinsic_height(),
                |texture| texture.intrinsic_height(),
            )
        }

        fn intrinsic_aspect_ratio(&self) -> f64 {
            self.texture.borrow().as_ref().map_or_else(
                || self.parent_intrinsic_aspect_ratio(),
                |texture| texture.intrinsic_aspect_ratio(),
            )
        }
    }

    impl AlbumArt {
        fn is_loaded(&self) -> bool {
            self.texture.borrow().is_some()
        }
    }
}

glib::wrapper! {
    pub struct AlbumArt(ObjectSubclass<imp::AlbumArt>)
        @implements gdk::Paintable;
}

impl AlbumArt {
    pub fn new(session: &soup::Session, download_url: &str, cache_path: impl AsRef<Path>) -> Self {
        glib::Object::builder()
            .property("session", session)
            .property("download-url", download_url)
            .property("cache-file", gio::File::for_path(cache_path))
            .build()
    }

    pub fn uri(&self) -> String {
        if self.is_loaded() {
            let cache_file = self.cache_file();
            debug_assert!(cache_file.query_exists(gio::Cancellable::NONE));
            return cache_file.uri().into();
        }

        self.download_url()
    }

    fn set_texture(&self, texture: gdk::Texture) {
        let imp = self.imp();
        imp.texture.replace(Some(texture));
        self.invalidate_contents();
        self.notify_is_loaded();
    }

    async fn load_texture(&self) -> Result<()> {
        let cache_file = self.cache_file();

        match cache_file.load_bytes_future().await {
            Ok((ref bytes, _)) => {
                let texture =
                    gdk::Texture::from_bytes(bytes).context("Failed to load texture from bytes")?;
                self.set_texture(texture);
                return Ok(());
            }
            Err(err) => {
                if !err.matches(gio::IOErrorEnum::NotFound) {
                    return Err(err.into());
                }

                tracing::debug!(
                    uri = ?cache_file.uri(),
                    "Cache file not found; downloading album art",
                );
            }
        }

        let download_url = self.download_url();

        let bytes = self
            .session()
            .send_and_read_future(
                &soup::Message::new("GET", &download_url)?,
                glib::PRIORITY_LOW,
            )
            .await
            .context("Failed to download album art")?;
        tracing::debug!(download_url, "Downloaded album art");

        let texture =
            gdk::Texture::from_bytes(&bytes).context("Failed to load texture from bytes")?;
        self.set_texture(texture.clone());

        cache_file
            .replace_contents_future(
                texture.save_to_png_bytes(),
                None,
                false,
                gio::FileCreateFlags::NONE,
            )
            .await
            .map_err(|(_, err)| err)
            .context("Failed to save texture to file")?;

        Ok(())
    }
}
