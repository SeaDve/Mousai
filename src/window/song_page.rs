use gettextrs::gettext;
use gtk::{
    gio,
    glib::{self, clone},
    prelude::*,
    subclass::prelude::*,
};
use once_cell::unsync::OnceCell;

use std::cell::RefCell;

use super::{
    album_cover::AlbumCover,
    external_link_tile::ExternalLinkTile,
    information_row::InformationRow,
    playback_button::{PlaybackButton, PlaybackButtonMode},
};
use crate::{
    model::{ExternalLinkWrapper, Song},
    song_player::{PlayerState, SongPlayer},
    spawn, Application,
};

mod imp {
    use super::*;
    use glib::{subclass::Signal, WeakRef};
    use gtk::CompositeTemplate;
    use once_cell::sync::Lazy;

    #[derive(Debug, Default, CompositeTemplate)]
    #[template(resource = "/io/github/seadve/Mousai/ui/song-page.ui")]
    pub struct SongPage {
        #[template_child]
        pub album_cover: TemplateChild<AlbumCover>,
        #[template_child]
        pub playback_button: TemplateChild<PlaybackButton>,
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
                    Application::default().show_error(&err.to_string());
                }
            });

            klass.install_action("song-page.remove-song", None, |obj, _, _| {
                if let Some(ref song) = obj.song() {
                    obj.emit_by_name::<()>("song-removed", &[song]);
                    obj.activate_action("win.navigate-to-main-page", None)
                        .unwrap();
                    obj.set_song(None);
                }
            });
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for SongPage {
        fn signals() -> &'static [Signal] {
            static SIGNALS: Lazy<Vec<Signal>> = Lazy::new(|| {
                vec![Signal::builder(
                    "song-removed",
                    &[Song::static_type().into()],
                    <()>::static_type().into(),
                )
                .build()]
            });
            SIGNALS.as_ref()
        }

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
                    if let Some(external_link_tile) = box_child
                        .child()
                        .and_then(|child| child.downcast::<ExternalLinkTile>().ok())
                    {
                        spawn!(async move {
                            let external_link_wrapper = external_link_tile.external_link();
                            let external_link = external_link_wrapper.inner();
                            let uri = external_link.uri();

                            if let Err(err) = gio::AppInfo::launch_default_for_uri_future(&uri, gio::AppLaunchContext::NONE).await
                            {
                                log::warn!("Failed to launch default for uri `{uri}`: {err:?}");
                                Application::default().show_error(&gettext!("Failed to launch {}", external_link.name()));
                            }
                        });
                    } else {
                        log::error!("Failed to activate external link: The FlowBoxChild does not have a child of ExternalLinkTile");
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

    pub fn connect_song_removed<F>(&self, f: F) -> glib::SignalHandlerId
    where
        F: Fn(&Self, &Song) + 'static,
    {
        self.connect_local("song-removed", true, move |values| {
            let obj = values[0].get::<Self>().unwrap();
            let song = values[1].get::<Song>().unwrap();
            f(&obj, &song);
            None
        })
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

        self.imp().player.set(player.downgrade()).unwrap();

        self.update_playback_ui();
    }

    fn toggle_playback(&self) -> anyhow::Result<()> {
        if let Some(ref player) = self.imp().player.get().and_then(|player| player.upgrade()) {
            if let Some(song) = self.song() {
                if player.state() == PlayerState::Playing && player.is_active_song(&song) {
                    player.pause();
                } else {
                    player.set_song(Some(song))?;
                    player.play();
                }
            }
        }

        Ok(())
    }

    fn update_playback_ui(&self) {
        let imp = self.imp();
        let song = self.song();

        imp.playback_button.set_visible(
            song.as_ref()
                .and_then(|song| song.playback_link())
                .is_some(),
        );

        if let Some(ref song) = song {
            if let Some(player) = imp.player.get().and_then(|player| player.upgrade()) {
                let is_active_song = player.is_active_song(song);
                let player_state = player.state();

                if is_active_song && player_state == PlayerState::Buffering {
                    imp.playback_button.set_mode(PlaybackButtonMode::Buffering);
                } else if is_active_song && player_state == PlayerState::Playing {
                    imp.playback_button.set_mode(PlaybackButtonMode::Pause);
                } else {
                    imp.playback_button.set_mode(PlaybackButtonMode::Play);
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
                    ExternalLinkTile::new(wrapper).upcast()
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
