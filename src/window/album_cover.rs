use gtk::{
    glib::{self, clone},
    prelude::*,
    subclass::prelude::*,
};

use std::cell::RefCell;

use crate::{core::AlbumArt, model::Song};

const DEFAULT_ENABLE_CROSSFADE: bool = true;

mod imp {
    use super::*;
    use std::marker::PhantomData;

    #[derive(Debug, Default, glib::Properties, gtk::CompositeTemplate)]
    #[properties(wrapper_type = super::AlbumCover)]
    #[template(resource = "/io/github/seadve/Mousai/ui/album-cover.ui")]
    pub struct AlbumCover {
        #[property(get = Self::pixel_size, set = Self::set_pixel_size, minimum = -1, default = -1, explicit_notify)]
        pub(super) pixel_size: PhantomData<i32>,
        #[property(get = Self::enables_crossfade, set = Self::set_enables_crossfade, default = DEFAULT_ENABLE_CROSSFADE, explicit_notify)]
        pub(super) enables_crossfade: PhantomData<bool>,

        #[template_child]
        pub(super) stack: TemplateChild<gtk::Stack>,
        #[template_child]
        pub(super) image_a: TemplateChild<gtk::Image>,
        #[template_child]
        pub(super) image_b: TemplateChild<gtk::Image>,
        #[template_child]
        pub(super) placeholder: TemplateChild<gtk::Image>,

        pub(super) album_art: RefCell<Option<AlbumArt>>,
        pub(super) album_art_is_loaded_notify_handler_id: RefCell<Option<glib::SignalHandlerId>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for AlbumCover {
        const NAME: &'static str = "MsaiAlbumCover";
        type Type = super::AlbumCover;
        type ParentType = gtk::Widget;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();

            klass.set_accessible_role(gtk::AccessibleRole::Img);
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for AlbumCover {
        crate::derived_properties!();

        fn constructed(&self) {
            self.parent_constructed();

            self.obj().set_enables_crossfade(DEFAULT_ENABLE_CROSSFADE);
        }

        fn dispose(&self) {
            self.dispose_template();
        }
    }

    impl WidgetImpl for AlbumCover {}

    impl AlbumCover {
        fn pixel_size(&self) -> i32 {
            let image_a_pixel_size = self.image_a.pixel_size();
            let image_b_pixel_size = self.image_b.pixel_size();
            debug_assert_eq!(
                image_a_pixel_size, image_b_pixel_size,
                "pixel sizes must be synced"
            );

            self.image_a.pixel_size()
        }

        fn set_pixel_size(&self, pixel_size: i32) {
            self.image_a.set_pixel_size(pixel_size);
            self.image_b.set_pixel_size(pixel_size);
            self.placeholder.set_pixel_size(pixel_size / 3);
            self.obj().notify_pixel_size();
        }

        fn enables_crossfade(&self) -> bool {
            self.stack.transition_type() == gtk::StackTransitionType::Crossfade
        }

        fn set_enables_crossfade(&self, enable_crossfade: bool) {
            self.stack.set_transition_type(if enable_crossfade {
                gtk::StackTransitionType::Crossfade
            } else {
                gtk::StackTransitionType::None
            });
            self.obj().notify_enables_crossfade();
        }
    }
}

glib::wrapper! {
    pub struct AlbumCover(ObjectSubclass<imp::AlbumCover>)
        @extends gtk::Widget;
}

impl AlbumCover {
    pub fn new() -> Self {
        glib::Object::new()
    }

    pub fn set_song(&self, song: Option<&Song>) {
        let imp = self.imp();

        if let Some(album_art) = song.as_ref().and_then(|song| song.album_art()) {
            match album_art {
                Ok(album_art) => {
                    if let Some(old_album_art) = self.album_art() {
                        if let Some(handler_id) = imp.album_art_is_loaded_notify_handler_id.take() {
                            old_album_art.disconnect(handler_id);
                        }
                    }

                    if album_art.is_loaded() {
                        self.set_album_art(Some(album_art));
                    } else {
                        imp.album_art_is_loaded_notify_handler_id.replace(Some(
                            album_art.connect_is_loaded_notify(
                                clone!(@weak self as obj => move |album_art| {
                                    if album_art.is_loaded() {
                                        obj.set_album_art(Some(album_art.clone()));
                                    }
                                }),
                            ),
                        ));
                    }
                }
                Err(err) => {
                    tracing::warn!("Failed to get song album art: {:?}", err);
                    self.set_album_art(None);
                }
            }
        } else {
            self.set_album_art(None);
        }
    }

    fn set_album_art(&self, paintable: Option<AlbumArt>) {
        let imp = self.imp();

        if let Some(ref paintable) = paintable {
            if imp.stack.visible_child().as_ref() == Some(imp.image_a.upcast_ref()) {
                imp.image_b.set_paintable(Some(paintable));
                imp.stack.set_visible_child(&imp.image_b.get());
            } else {
                imp.image_a.set_paintable(Some(paintable));
                imp.stack.set_visible_child(&imp.image_a.get());
            }
        } else {
            imp.stack.set_visible_child(&imp.placeholder.get());
        }

        imp.album_art.replace(paintable);
    }

    fn album_art(&self) -> Option<AlbumArt> {
        let imp = self.imp();
        let album_art = imp.album_art.borrow().clone();

        debug_assert_eq!(
            album_art,
            imp.stack.visible_child().and_then(|visible_child| {
                if &visible_child == imp.image_a.upcast_ref::<gtk::Widget>() {
                    imp.image_a.paintable().map(|p| p.downcast().unwrap())
                } else if &visible_child == imp.image_b.upcast_ref::<gtk::Widget>() {
                    imp.image_b.paintable().map(|p| p.downcast().unwrap())
                } else {
                    None
                }
            })
        );

        album_art
    }
}

impl Default for AlbumCover {
    fn default() -> Self {
        Self::new()
    }
}
