use adw::prelude::*;
use gtk::{
    gdk,
    glib::{self, clone},
    subclass::prelude::*,
};
use once_cell::unsync::OnceCell;

use std::cell::{Cell, RefCell};

use crate::{model::Song, utils};

const FADE_ANIMATION_DURATION_MS: u32 = 800;

mod imp {
    use super::*;

    #[derive(Debug, Default, glib::Properties)]
    #[properties(wrapper_type = super::CrossfadePaintable)]
    pub struct CrossfadePaintable {
        #[property(get, set, construct_only)]
        pub(super) widget: glib::WeakRef<gtk::Widget>,
        #[property(get, set = Self::set_paintable, explicit_notify, nullable)]
        pub(super) paintable: RefCell<Option<gdk::Paintable>>,

        pub(super) paintable_invalidate_contents_handler_id: RefCell<Option<glib::SignalHandlerId>>,
        pub(super) paintable_invalidate_size_handler_id: RefCell<Option<glib::SignalHandlerId>>,

        pub(super) prev_paintable: RefCell<Option<gdk::Paintable>>,
        pub(super) prev_paintable_invalidate_contents_handler_id:
            RefCell<Option<glib::SignalHandlerId>>,

        pub(super) fade_progress: Cell<f64>,
        pub(super) fade_animation: OnceCell<adw::TimedAnimation>,

        pub(super) join_handle: RefCell<Option<glib::JoinHandle<()>>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for CrossfadePaintable {
        const NAME: &'static str = "MsaiCrossfadePaintable";
        type Type = super::CrossfadePaintable;
        type Interfaces = (gdk::Paintable,);
    }

    impl ObjectImpl for CrossfadePaintable {
        fn constructed(&self) {
            self.parent_constructed();

            let obj = self.obj();
            let widget = self.widget.upgrade().expect("widget must be alive");

            let target = adw::CallbackAnimationTarget::new(clone!(@weak obj => move |value| {
                obj.imp().fade_progress.set(value);
                obj.invalidate_contents();
            }));
            let fade_animation = adw::TimedAnimation::builder()
                .widget(&widget)
                .value_from(0.0)
                .value_to(1.0)
                .duration(FADE_ANIMATION_DURATION_MS)
                .target(&target)
                .build();
            self.fade_animation.set(fade_animation).unwrap();
        }

        crate::derived_properties!();
    }

    impl PaintableImpl for CrossfadePaintable {
        fn snapshot(&self, snapshot: &gdk::Snapshot, width: f64, height: f64) {
            let fade_progress = self.fade_progress.get();
            snapshot.push_cross_fade(fade_progress);

            if let Some(paintable) = self.prev_paintable.borrow().as_ref() {
                paintable.snapshot(snapshot, width, height);
            }
            snapshot.pop();

            if let Some(paintable) = self.paintable.borrow().as_ref() {
                paintable.snapshot(snapshot, width, height);
            }
            snapshot.pop();
        }

        fn current_image(&self) -> gdk::Paintable {
            self.paintable.borrow().as_ref().map_or_else(
                || self.parent_current_image(),
                |paintable| paintable.current_image(),
            )
        }

        fn flags(&self) -> gdk::PaintableFlags {
            self.paintable.borrow().as_ref().map_or_else(
                || self.parent_flags(),
                |paintable| {
                    let mut flags = paintable.flags();
                    flags.remove(gdk::PaintableFlags::CONTENTS);
                    flags
                },
            )
        }

        fn intrinsic_width(&self) -> i32 {
            self.paintable.borrow().as_ref().map_or_else(
                || self.parent_intrinsic_width(),
                |paintable| paintable.intrinsic_width(),
            )
        }

        fn intrinsic_height(&self) -> i32 {
            self.paintable.borrow().as_ref().map_or_else(
                || self.parent_intrinsic_height(),
                |paintable| paintable.intrinsic_height(),
            )
        }

        fn intrinsic_aspect_ratio(&self) -> f64 {
            self.paintable.borrow().as_ref().map_or_else(
                || self.parent_intrinsic_aspect_ratio(),
                |paintable| paintable.intrinsic_aspect_ratio(),
            )
        }
    }

    impl CrossfadePaintable {
        fn set_paintable(&self, paintable: Option<gdk::Paintable>) {
            let obj = self.obj();

            let prev_paintable = self.paintable.replace(paintable.clone());

            if prev_paintable == paintable {
                return;
            }

            let prev_prev_paintable = self.prev_paintable.replace(prev_paintable.clone());

            if let Some(ref prev_paintable) = prev_paintable {
                if let Some(handler_id) = self.paintable_invalidate_contents_handler_id.take() {
                    prev_paintable.disconnect(handler_id);
                }

                if let Some(handler_id) = self.paintable_invalidate_size_handler_id.take() {
                    prev_paintable.disconnect(handler_id);
                }
            }

            if let Some(paintable) = paintable {
                self.paintable_invalidate_contents_handler_id.replace(Some(
                    paintable.connect_invalidate_contents(clone!(@weak obj => move |_| {
                        obj.invalidate_contents();
                    })),
                ));

                self.paintable_invalidate_size_handler_id.replace(Some(
                    paintable.connect_invalidate_size(clone!(@weak obj => move |_| {
                        obj.invalidate_size();
                    })),
                ));
            }

            if let Some(prev_prev_paintable) = prev_prev_paintable {
                if let Some(handler_id) = self.prev_paintable_invalidate_contents_handler_id.take()
                {
                    prev_prev_paintable.disconnect(handler_id);
                }
            }

            if let Some(prev_paintable) = prev_paintable {
                self.prev_paintable_invalidate_contents_handler_id
                    .replace(Some(prev_paintable.connect_invalidate_contents(
                        clone!(@weak obj => move |_| {
                            obj.invalidate_contents();
                        }),
                    )));
            }

            let fade_animation = self.fade_animation.get().unwrap();
            fade_animation.pause();
            fade_animation.set_value_from(1.0 - self.fade_progress.get());
            fade_animation.play();

            obj.notify_paintable();
        }
    }
}

glib::wrapper! {
    /// Adds crossfade when switching between paintables.
    pub struct CrossfadePaintable(ObjectSubclass<imp::CrossfadePaintable>)
        @implements gdk::Paintable;
}

impl CrossfadePaintable {
    pub fn new(widget: &impl IsA<gtk::Widget>) -> Self {
        glib::Object::builder().property("widget", widget).build()
    }

    /// Helper to set the album art of the song as the paintable.
    pub fn set_song(&self, song: Option<&Song>) {
        let imp = self.imp();

        if let Some(join_handle) = imp.join_handle.take() {
            join_handle.abort();
        }

        if let Some(album_art) = song.and_then(|song| song.album_art()) {
            match album_art {
                Ok(album_art) => {
                    let join_handle =
                        utils::spawn(clone!(@weak self as obj, @weak album_art => async move {
                            match album_art.texture().await {
                                Ok(texture) => {
                                    obj.set_paintable(Some(texture.upcast_ref::<gdk::Paintable>()));
                                }
                                Err(err) => {
                                    tracing::warn!("Failed to load texture: {:?}", err);
                                    obj.set_paintable(gdk::Paintable::NONE);
                                }
                            }
                        }));
                    imp.join_handle.replace(Some(join_handle));
                }
                Err(err) => {
                    tracing::warn!("Failed to get song album art: {:?}", err);
                    self.set_paintable(gdk::Paintable::NONE);
                }
            }
        } else {
            self.set_paintable(gdk::Paintable::NONE);
        }
    }
}
