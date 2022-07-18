use gtk::{
    gdk,
    glib::{self, clone},
    prelude::*,
    subclass::prelude::*,
};

use std::cell::RefCell;

use crate::{model::Song, utils};

const DEFAULT_ENABLE_CROSSFADE: bool = true;

mod imp {
    use super::*;
    use gtk::CompositeTemplate;
    use once_cell::sync::Lazy;

    #[derive(Debug, Default, CompositeTemplate)]
    #[template(resource = "/io/github/seadve/Mousai/ui/album-cover.ui")]
    pub struct AlbumCover {
        #[template_child]
        pub stack: TemplateChild<gtk::Stack>,
        #[template_child]
        pub image_a: TemplateChild<gtk::Image>,
        #[template_child]
        pub image_b: TemplateChild<gtk::Image>,
        #[template_child]
        pub placeholder: TemplateChild<gtk::Image>,

        pub song: RefCell<Option<Song>>,
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
        fn properties() -> &'static [glib::ParamSpec] {
            static PROPERTIES: Lazy<Vec<glib::ParamSpec>> = Lazy::new(|| {
                vec![
                    // Song represented by Self
                    glib::ParamSpecObject::builder("song", Song::static_type())
                        .flags(glib::ParamFlags::READWRITE | glib::ParamFlags::EXPLICIT_NOTIFY)
                        .build(),
                    // Pixel Size of the inner GtkImage
                    glib::ParamSpecInt::builder("pixel-size")
                        .minimum(-1)
                        .default_value(-1)
                        .flags(glib::ParamFlags::READWRITE | glib::ParamFlags::EXPLICIT_NOTIFY)
                        .build(),
                    // Whether to animate when switching between textures
                    glib::ParamSpecBoolean::builder("enable-crossfade")
                        .default_value(DEFAULT_ENABLE_CROSSFADE)
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
                "song" => {
                    let song = value.get().unwrap();
                    obj.set_song(song);
                }
                "pixel-size" => {
                    let pixel_size = value.get().unwrap();
                    obj.set_pixel_size(pixel_size);
                }
                "enable-crossfade" => {
                    let enable_crossfade = value.get().unwrap();
                    obj.set_enable_crossfade(enable_crossfade);
                }
                _ => unimplemented!(),
            }
        }

        fn property(&self, obj: &Self::Type, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
            match pspec.name() {
                "song" => obj.song().to_value(),
                "pixel-size" => obj.pixel_size().to_value(),
                "enable-crossfade" => obj.enable_crossfade().to_value(),
                _ => unimplemented!(),
            }
        }

        fn constructed(&self, obj: &Self::Type) {
            self.parent_constructed(obj);

            obj.set_enable_crossfade(DEFAULT_ENABLE_CROSSFADE);
        }

        fn dispose(&self, obj: &Self::Type) {
            while let Some(child) = obj.first_child() {
                child.unparent();
            }
        }
    }

    impl WidgetImpl for AlbumCover {}
}

glib::wrapper! {
    pub struct AlbumCover(ObjectSubclass<imp::AlbumCover>)
        @extends gtk::Widget;
}

impl AlbumCover {
    pub fn new() -> Self {
        glib::Object::new(&[]).expect("Failed to create AlbumCover")
    }

    pub fn set_song(&self, song: Option<Song>) {
        if song == self.song() {
            return;
        }

        if let Some(ref song) = song {
            match song.album_art() {
                Ok(album_art) => {
                    utils::spawn(clone!(@weak self as obj, @weak album_art => async move {
                        match album_art.texture().await {
                            Ok(texture) => {
                                obj.set_paintable(texture);
                            }
                            Err(err) => {
                                log::warn!("Failed to load texture: {err:?}");
                                obj.clear();
                            }
                        }
                    }));
                }
                Err(err) => {
                    log::warn!("Failed to get song album art: {err:?}");
                    self.clear();
                }
            }
        } else {
            self.clear();
        }

        self.imp().song.replace(song);
        self.notify("song");
    }

    pub fn song(&self) -> Option<Song> {
        self.imp().song.borrow().clone()
    }

    pub fn set_pixel_size(&self, pixel_size: i32) {
        let imp = self.imp();
        imp.image_a.set_pixel_size(pixel_size);
        imp.image_b.set_pixel_size(pixel_size);
        imp.placeholder.set_pixel_size(pixel_size / 3);
        self.notify("pixel-size");
    }

    pub fn pixel_size(&self) -> i32 {
        self.imp().image_a.pixel_size()
    }

    pub fn set_enable_crossfade(&self, enable_crossfade: bool) {
        self.imp().stack.set_transition_type(if enable_crossfade {
            gtk::StackTransitionType::Crossfade
        } else {
            gtk::StackTransitionType::None
        });
        self.notify("enable-crossfade");
    }

    pub fn enable_crossfade(&self) -> bool {
        self.imp().stack.transition_type() == gtk::StackTransitionType::Crossfade
    }

    fn clear(&self) {
        let imp = self.imp();
        imp.image_a.set_paintable(gdk::Paintable::NONE);
        imp.image_b.set_paintable(gdk::Paintable::NONE);
        imp.stack.set_visible_child(&imp.placeholder.get());
    }

    fn set_paintable(&self, paintable: &impl IsA<gdk::Paintable>) {
        let imp = self.imp();

        if imp.stack.visible_child().as_ref() == Some(imp.image_a.upcast_ref()) {
            imp.image_b.set_paintable(Some(paintable));
            imp.stack.set_visible_child(&imp.image_b.get());
        } else {
            imp.image_a.set_paintable(Some(paintable));
            imp.stack.set_visible_child(&imp.image_a.get());
        }
    }
}

impl Default for AlbumCover {
    fn default() -> Self {
        Self::new()
    }
}
