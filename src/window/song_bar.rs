use gtk::{
    glib::{self, clone},
    prelude::*,
    subclass::prelude::*,
};
use once_cell::unsync::OnceCell;

use std::{cell::RefCell, time::Duration};

use super::{
    album_cover::AlbumCover,
    playback_button::{PlaybackButton, PlaybackButtonMode},
    time_label::TimeLabel,
};
use crate::{
    core::ClockTime,
    model::Song,
    song_player::{PlayerState, SongPlayer},
};

mod imp {
    use super::*;
    use glib::subclass::Signal;
    use gtk::CompositeTemplate;
    use once_cell::sync::Lazy;

    #[derive(Debug, Default, CompositeTemplate)]
    #[template(resource = "/io/github/seadve/Mousai/ui/song-bar.ui")]
    pub struct SongBar {
        #[template_child]
        pub album_cover: TemplateChild<AlbumCover>,
        #[template_child]
        pub playback_button: TemplateChild<PlaybackButton>,
        #[template_child]
        pub playback_position_scale: TemplateChild<gtk::Scale>,
        #[template_child]
        pub playback_position_label: TemplateChild<TimeLabel>,
        #[template_child]
        pub duration_label: TemplateChild<TimeLabel>,

        pub song: RefCell<Option<Song>>,
        pub scale_handler_id: OnceCell<glib::SignalHandlerId>,
        pub seek_timeout_id: RefCell<Option<glib::SourceId>>,
        pub player: OnceCell<SongPlayer>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for SongBar {
        const NAME: &'static str = "MsaiSongBar";
        type Type = super::SongBar;
        type ParentType = gtk::Widget;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();

            klass.install_action("song-bar.clear", None, |obj, _, _| {
                if let Err(err) = obj.set_song(None) {
                    log::info!("Failed to clear SongBar song: {err:?}");
                }
            });

            klass.install_action("song-bar.activate-song", None, |obj, _, _| {
                if let Some(ref song) = obj.song() {
                    obj.emit_by_name::<()>("song-activated", &[song]);
                }
            });
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for SongBar {
        fn signals() -> &'static [Signal] {
            static SIGNALS: Lazy<Vec<Signal>> = Lazy::new(|| {
                vec![Signal::builder(
                    "song-activated",
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
                    if let Err(err) = obj.set_song(song) {
                        log::warn!("Failed to set song to SongBar: {err:?}");
                    }
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

            self.scale_handler_id
                .set(self.playback_position_scale.connect_value_changed(
                    clone!(@weak obj => move |scale| {
                        obj.on_playback_position_scale_value_changed(scale);
                    }),
                ))
                .unwrap();

            obj.update_album_cover();
            obj.update_actions_sensitivity();
        }

        fn dispose(&self, obj: &Self::Type) {
            while let Some(child) = obj.first_child() {
                child.unparent();
            }
        }
    }

    impl WidgetImpl for SongBar {}
}

glib::wrapper! {
    pub struct SongBar(ObjectSubclass<imp::SongBar>)
        @extends gtk::Widget;
}

impl SongBar {
    pub fn new() -> Self {
        glib::Object::new(&[]).expect("Failed to create SongBar")
    }

    pub fn connect_song_activated<F>(&self, f: F) -> glib::SignalHandlerId
    where
        F: Fn(&Self, &Song) + 'static,
    {
        self.connect_local("song-activated", true, move |values| {
            let obj = values[0].get::<Self>().unwrap();
            let song = values[1].get::<Song>().unwrap();
            f(&obj, &song);
            None
        })
    }

    pub fn set_song(&self, song: Option<Song>) -> anyhow::Result<()> {
        if song == self.song() {
            return Ok(());
        }

        let imp = self.imp();

        self.player().set_song(song.clone())?;
        imp.song.replace(song);

        self.update_album_cover();
        self.update_actions_sensitivity();

        self.notify("song");

        Ok(())
    }

    pub fn song(&self) -> Option<Song> {
        self.imp().song.borrow().clone()
    }

    /// Must only be called once.
    pub fn bind_player(&self, player: &SongPlayer) {
        let imp = self.imp();

        imp.player.set(player.clone()).unwrap();

        player
            .bind_property("song", self, "song")
            .flags(glib::BindingFlags::SYNC_CREATE)
            .build();

        player.connect_state_notify(clone!(@weak self as obj => move |_| {
            obj.update_playback_button();
        }));

        player.connect_position_notify(clone!(@weak self as obj => move |_| {
            obj.update_position_ui();
        }));

        player.connect_duration_notify(clone!(@weak self as obj => move |_| {
            obj.update_duration_ui();
        }));

        self.update_playback_button();
        self.update_position_ui();
        self.update_duration_ui();
    }

    fn player(&self) -> &SongPlayer {
        self.imp().player.get_or_init(|| {
            log::error!("SongPlayer was not bound in SongBar. Creating a default one.");
            SongPlayer::default()
        })
    }

    fn set_playback_position_scale_value_blocking(&self, value: f64) {
        let imp = self.imp();
        let scale_handler_id = imp.scale_handler_id.get().unwrap();
        imp.playback_position_scale.block_signal(scale_handler_id);
        imp.playback_position_scale.set_value(value);
        imp.playback_position_scale.unblock_signal(scale_handler_id);
    }

    fn on_playback_position_scale_value_changed(&self, scale: &gtk::Scale) {
        let imp = self.imp();

        // Cancel the seek when the value changed again within 20ms. So, it
        // will only seek when the value is stabilized within that span.
        if let Some(source_id) = imp.seek_timeout_id.take() {
            source_id.remove();
        }

        let value = scale.value();

        imp.seek_timeout_id
            .replace(Some(glib::timeout_add_local_once(
                Duration::from_millis(20),
                clone!(@weak self as obj => move || {
                    obj.imp().seek_timeout_id.replace(None);
                    if let Err(err) = obj.player().seek(ClockTime::from_secs_f64(value)) {
                        log::warn!("Failed to seek to `{value}` secs: {err:?}");
                    }
                }),
            )));
    }

    fn update_position_ui(&self) {
        let position = self.player().position().unwrap_or_default();
        self.set_playback_position_scale_value_blocking(position.as_secs_f64());
        self.imp().playback_position_label.set_time(position);
    }

    fn update_duration_ui(&self) {
        let imp = self.imp();
        let duration = self.player().duration().unwrap_or_default();
        imp.playback_position_scale
            .set_range(0.0, duration.as_secs_f64());
        imp.duration_label.set_time(duration);
    }

    fn update_playback_button(&self) {
        let imp = self.imp();

        match self.player().state() {
            PlayerState::Buffering => {
                imp.playback_button.set_mode(PlaybackButtonMode::Buffering);
            }
            PlayerState::Stopped | PlayerState::Paused => {
                imp.playback_button.set_mode(PlaybackButtonMode::Play);
            }
            PlayerState::Playing => {
                imp.playback_button.set_mode(PlaybackButtonMode::Pause);
            }
        }
    }

    fn update_album_cover(&self) {
        self.imp().album_cover.set_song(self.song());
    }

    fn update_actions_sensitivity(&self) {
        let has_song = self.song().is_some();
        self.imp().playback_position_scale.set_sensitive(has_song);
        self.action_set_enabled("song-bar.clear", has_song);
    }
}

impl Default for SongBar {
    fn default() -> Self {
        Self::new()
    }
}
