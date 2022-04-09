use adw::prelude::*;
use gettextrs::gettext;
use gtk::{
    glib::{self, clone},
    subclass::prelude::*,
};
use once_cell::unsync::OnceCell;

use std::cell::RefCell;

use super::{
    album_cover::AlbumCover, external_link_cell::ExternalLinkCell, information_row::InformationRow,
};
use crate::{
    core::PlaybackState,
    model::{ExternalLinkWrapper, Song},
    song_player::SongPlayer,
    Application,
};

mod imp {
    use super::*;
    use glib::WeakRef;
    use gtk::CompositeTemplate;
    use once_cell::sync::Lazy;

    #[derive(Debug, Default, CompositeTemplate)]
    #[template(resource = "/io/github/seadve/Mousai/ui/song-page.ui")]
    pub struct SongPage {
        #[template_child]
        pub album_cover: TemplateChild<AlbumCover>,
        #[template_child]
        pub playback_stack: TemplateChild<gtk::Stack>,
        #[template_child]
        pub toggle_playback_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub buffering_spinner: TemplateChild<gtk::Spinner>,
        #[template_child]
        pub last_heard_row: TemplateChild<InformationRow>,
        #[template_child]
        pub album_row: TemplateChild<InformationRow>,
        #[template_child]
        pub release_date_row: TemplateChild<InformationRow>,
        #[template_child]
        pub external_links_box: TemplateChild<gtk::FlowBox>,

        pub song: RefCell<Option<Song>>,
        pub player: OnceCell<WeakRef<SongPlayer>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for SongPage {
        const NAME: &'static str = "MsaiSongPage";
        type Type = super::SongPage;
        type ParentType = gtk::Widget;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();

            klass.install_action("song-page.toggle-playback", None, |obj, _, _| {
                if let Err(err) = obj.toggle_playback() {
                    log::warn!("Failed to toggle playback: {err:?}");
                    if let Some(window) = Application::default().main_window() {
                        window.show_error(&err.to_string());
                    }
                }
            });
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for SongPage {
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

            obj.add_css_class("view");

            self.external_links_box
                .connect_child_activated(|_, box_child| {
                    if let Some(external_link_cell) = box_child
                        .child()
                        .and_then(|child| child.downcast::<ExternalLinkCell>().ok())
                    {
                        external_link_cell.external_link().inner().activate();
                    }
                });
        }

        fn dispose(&self, obj: &Self::Type) {
            while let Some(child) = obj.first_child() {
                child.unparent();
            }
        }
    }

    impl WidgetImpl for SongPage {}
}

glib::wrapper! {
    pub struct SongPage(ObjectSubclass<imp::SongPage>)
        @extends gtk::Widget;
}

impl SongPage {
    pub fn new() -> Self {
        glib::Object::new(&[]).expect("Failed to create SongPage")
    }

    pub fn set_song(&self, song: Option<Song>) {
        if song == self.song() {
            return;
        }

        let imp = self.imp();
        imp.song.replace(song.clone());
        imp.album_cover.set_song(song);
        self.update_information();
        self.update_playback_ui();
        self.update_external_links();

        self.notify("song");
    }

    pub fn song(&self) -> Option<Song> {
        self.imp().song.borrow().clone()
    }

    /// Must only be called once.
    pub fn bind_player(&self, player: &SongPlayer) {
        player.connect_state_notify(clone!(@weak self as obj => move |_| {
            obj.update_playback_ui();
        }));

        player.connect_is_buffering_notify(clone!(@weak self as obj => move |_| {
            obj.update_playback_ui();
        }));

        self.imp().player.set(player.downgrade()).unwrap();

        self.update_playback_ui();
    }

    fn toggle_playback(&self) -> anyhow::Result<()> {
        if let Some(ref player) = self.imp().player.get().and_then(|player| player.upgrade()) {
            if let Some(song) = self.song() {
                if player.state() == PlaybackState::Playing && player.is_active_song(&song) {
                    player.pause()?;
                } else {
                    player.set_song(Some(song))?;
                    player.play()?;
                }
            }
        }

        Ok(())
    }

    fn update_playback_ui(&self) {
        let imp = self.imp();
        let song = self.song();

        imp.playback_stack.set_visible(
            song.as_ref()
                .and_then(|song| song.playback_link())
                .is_some(),
        );

        if let Some(ref song) = song {
            if let Some(player) = imp.player.get().and_then(|player| player.upgrade()) {
                let toggle_playback_button = &imp.toggle_playback_button.get();
                let buffering_spinner = &imp.buffering_spinner.get();

                let is_active_song = player.is_active_song(song);

                if is_active_song && player.is_buffering() {
                    buffering_spinner.set_spinning(true);
                    imp.playback_stack.set_visible_child(buffering_spinner);
                    return;
                }

                imp.playback_stack.set_visible_child(toggle_playback_button);
                buffering_spinner.set_spinning(false);

                if is_active_song && player.state() == PlaybackState::Playing {
                    toggle_playback_button.set_icon_name("media-playback-pause-symbolic");
                    toggle_playback_button.set_tooltip_text(Some(&gettext("Pause")));
                } else {
                    toggle_playback_button.set_icon_name("media-playback-start-symbolic");
                    toggle_playback_button.set_tooltip_text(Some(&gettext("Play")));
                }
            } else {
                log::error!("Either the player was dropped or not binded in SongPage");
            }
        }
    }

    fn update_external_links(&self) {
        if let Some(song) = self.song() {
            self.imp()
                .external_links_box
                .bind_model(Some(&song.external_links()), |item| {
                    let wrapper: &ExternalLinkWrapper = item.downcast_ref().unwrap();
                    ExternalLinkCell::new(wrapper).upcast()
                });
        }
    }

    fn update_information(&self) {
        let song = match self.song() {
            Some(song) => song,
            None => return,
        };

        let imp = self.imp();

        imp.last_heard_row
            .set_data(&song.last_heard().fuzzy_display());
        imp.album_row.set_data(&song.album());
        imp.release_date_row.set_data(&song.release_date());
    }
}

impl Default for SongPage {
    fn default() -> Self {
        Self::new()
    }
}
