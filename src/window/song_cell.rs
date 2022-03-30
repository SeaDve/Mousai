use gtk::{glib, prelude::*, subclass::prelude::*};

use std::cell::RefCell;

use super::{album_art::AlbumArt, Window};
use crate::model::Song;

mod imp {
    use super::*;
    use gtk::CompositeTemplate;
    use once_cell::sync::Lazy;

    #[derive(Debug, Default, CompositeTemplate)]
    #[template(resource = "/io/github/seadve/Mousai/ui/song-cell.ui")]
    pub struct SongCell {
        #[template_child]
        pub album_art: TemplateChild<AlbumArt>,
        #[template_child]
        pub toggle_playback_button: TemplateChild<gtk::Button>,

        pub song: RefCell<Option<Song>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for SongCell {
        const NAME: &'static str = "MsaiSongCell";
        type Type = super::SongCell;
        type ParentType = gtk::Widget;

        fn class_init(klass: &mut Self::Class) {
            Self::bind_template(klass);

            klass.install_action("song-cell.toggle-playback", None, move |obj, _, _| {
                if let Err(err) = obj.toggle_playback() {
                    log::warn!("Failed to toggle playback: {err:?}");
                }
            });
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for SongCell {
        fn properties() -> &'static [glib::ParamSpec] {
            static PROPERTIES: Lazy<Vec<glib::ParamSpec>> = Lazy::new(|| {
                vec![glib::ParamSpecObject::new(
                    "song",
                    "Song",
                    "Song represented by Self",
                    Song::static_type(),
                    glib::ParamFlags::READWRITE | glib::ParamFlags::EXPLICIT_NOTIFY,
                )]
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
                _ => unimplemented!(),
            }
        }

        fn property(&self, obj: &Self::Type, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
            match pspec.name() {
                "song" => obj.song().to_value(),
                _ => unimplemented!(),
            }
        }

        fn constructed(&self, obj: &Self::Type) {
            self.parent_constructed(obj);

            obj.update_toggle_playback_button_visibility();
        }

        fn dispose(&self, obj: &Self::Type) {
            while let Some(child) = obj.first_child() {
                child.unparent();
            }
        }
    }

    impl WidgetImpl for SongCell {}
}

glib::wrapper! {
    pub struct SongCell(ObjectSubclass<imp::SongCell>)
        @extends gtk::Widget;
}

impl SongCell {
    pub fn new() -> Self {
        glib::Object::new(&[]).expect("Failed to create SongCell")
    }

    pub fn set_song(&self, song: Option<Song>) {
        if song == self.song() {
            return;
        }

        let imp = self.imp();

        imp.album_art.set_song(song.clone());

        imp.song.replace(song);
        self.update_toggle_playback_button_visibility();

        self.notify("song");
    }

    pub fn song(&self) -> Option<Song> {
        self.imp().song.borrow().clone()
    }

    fn toggle_playback(&self) -> anyhow::Result<()> {
        if let Some(audio_player_widget) = self
            .root()
            .and_then(|root| root.downcast::<Window>().ok())
            .map(|window| window.audio_player_widget())
        {
            if let Some(song) = self.song() {
                audio_player_widget.set_song(Some(song))?;
                audio_player_widget.play()?;
            }
            Ok(())
        } else {
            Err(anyhow::anyhow!("AudioPlayerWidget was not found"))
        }
    }

    fn update_toggle_playback_button_visibility(&self) {
        self.imp()
            .toggle_playback_button
            .set_visible(self.song().and_then(|song| song.playback_link()).is_some());
    }
}

impl Default for SongCell {
    fn default() -> Self {
        Self::new()
    }
}
