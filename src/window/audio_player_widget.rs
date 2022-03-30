use gtk::{
    glib::{self, clone},
    prelude::*,
    subclass::prelude::*,
};
use once_cell::unsync::OnceCell;

use std::{cell::RefCell, time::Duration};

use super::{album_art::AlbumArt, time_label::TimeLabel};
use crate::{
    core::{AudioPlayer, ClockTime, PlaybackState},
    model::Song,
    spawn,
};

mod imp {
    use super::*;
    use gtk::CompositeTemplate;
    use once_cell::sync::Lazy;

    #[derive(Debug, Default, CompositeTemplate)]
    #[template(resource = "/io/github/seadve/Mousai/ui/audio-player-widget.ui")]
    pub struct AudioPlayerWidget {
        #[template_child]
        pub album_art: TemplateChild<AlbumArt>,
        #[template_child]
        pub buffering_spinner: TemplateChild<gtk::Spinner>,
        #[template_child]
        pub playback_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub playback_position_scale: TemplateChild<gtk::Scale>,
        #[template_child]
        pub playback_position_label: TemplateChild<TimeLabel>,
        #[template_child]
        pub duration_label: TemplateChild<TimeLabel>,

        pub song: RefCell<Option<Song>>,
        pub scale_handler_id: OnceCell<glib::SignalHandlerId>,
        pub seek_timeout_id: RefCell<Option<glib::SourceId>>,
        pub audio_player: AudioPlayer,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for AudioPlayerWidget {
        const NAME: &'static str = "MsaiAudioPlayerWidget";
        type Type = super::AudioPlayerWidget;
        type ParentType = gtk::Widget;

        fn class_init(klass: &mut Self::Class) {
            Self::bind_template(klass);

            klass.install_action(
                "audio-player-widget.toggle-playback",
                None,
                move |obj, _, _| {
                    if let Err(err) = obj.toggle_playback() {
                        log::warn!("Failed to toggle playback: {err:?}");
                    }
                },
            );

            klass.install_action("audio-player-widget.clear", None, move |obj, _, _| {
                if let Err(err) = obj.set_song(None) {
                    log::info!("Failed to clear AudioPlayerWidget song: {err:?}");
                }
            });
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for AudioPlayerWidget {
        fn properties() -> &'static [glib::ParamSpec] {
            static PROPERTIES: Lazy<Vec<glib::ParamSpec>> = Lazy::new(|| {
                vec![
                    glib::ParamSpecObject::new(
                        "song",
                        "Song",
                        "Song represented by Self",
                        Song::static_type(),
                        glib::ParamFlags::READWRITE | glib::ParamFlags::EXPLICIT_NOTIFY,
                    ),
                    glib::ParamSpecEnum::new(
                        "state",
                        "State",
                        "Current state of the widget",
                        PlaybackState::static_type(),
                        PlaybackState::default() as i32,
                        glib::ParamFlags::READABLE,
                    ),
                    glib::ParamSpecBoolean::new(
                        "is-buffering",
                        "Is Buffering",
                        "Whether this is buffering",
                        false,
                        glib::ParamFlags::READABLE,
                    ),
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
                    if let Err(err) = obj.set_song(song) {
                        log::warn!("Failed to set song to AudioPPLayerWidget: {err:?}");
                    }
                }
                _ => unimplemented!(),
            }
        }

        fn property(&self, obj: &Self::Type, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
            match pspec.name() {
                "song" => obj.song().to_value(),
                "state" => obj.state().to_value(),
                "is-buffering" => obj.is_buffering().to_value(),
                _ => unimplemented!(),
            }
        }

        fn constructed(&self, obj: &Self::Type) {
            self.parent_constructed(obj);

            self.audio_player
                .bind_property("is-buffering", &self.buffering_spinner.get(), "spinning")
                .flags(glib::BindingFlags::SYNC_CREATE)
                .build();

            self.audio_player
                .connect_state_notify(clone!(@weak obj => move |_| {
                    obj.notify("state");
                }));

            self.audio_player
                .connect_is_buffering_notify(clone!(@weak obj => move |_| {
                    obj.notify("is-buffering");
                }));

            obj.setup_audio_player();

            obj.update_playback_state_ui();
            obj.update_album_art();
            obj.update_actions_sensitivity();
        }

        fn dispose(&self, obj: &Self::Type) {
            while let Some(child) = obj.first_child() {
                child.unparent();
            }
        }
    }

    impl WidgetImpl for AudioPlayerWidget {}
}

glib::wrapper! {
    pub struct AudioPlayerWidget(ObjectSubclass<imp::AudioPlayerWidget>)
        @extends gtk::Widget;
}

impl AudioPlayerWidget {
    pub fn new() -> Self {
        glib::Object::new(&[]).expect("Failed to create AudioPlayerWidget")
    }

    pub fn set_song(&self, song: Option<Song>) -> anyhow::Result<()> {
        if song == self.song() {
            return Ok(());
        }

        let imp = self.imp();

        imp.audio_player.set_state(PlaybackState::Stopped);

        if let Some(ref song) = song {
            let playback_link = song.playback_link().ok_or_else(|| {
                anyhow::anyhow!("Trying to set a song to audio player without playback link")
            })?;
            imp.audio_player.set_uri(&playback_link)?;

            spawn!(
                glib::PRIORITY_DEFAULT_IDLE,
                clone!(@weak self as obj => async move {
                    if let Err(err) = obj.update_duration_label().await {
                        log::warn!("Failed to update playback duration label: {err:?}");
                    }
                })
            );
        } else {
            imp.playback_position_label.reset();
            imp.duration_label.reset();
        }

        imp.song.replace(song);

        self.update_album_art();
        self.update_actions_sensitivity();

        self.notify("song");

        Ok(())
    }

    pub fn song(&self) -> Option<Song> {
        self.imp().song.borrow().clone()
    }

    pub fn connect_state_notify<F>(&self, f: F) -> glib::SignalHandlerId
    where
        F: Fn(&Self) + 'static,
    {
        self.connect_notify_local(Some("state"), move |obj, _| f(obj))
    }

    pub fn state(&self) -> PlaybackState {
        self.imp().audio_player.state()
    }

    pub fn connect_is_buffering_notify<F>(&self, f: F) -> glib::SignalHandlerId
    where
        F: Fn(&Self) + 'static,
    {
        self.connect_notify_local(Some("is-buffering"), move |obj, _| f(obj))
    }

    pub fn is_buffering(&self) -> bool {
        self.imp().audio_player.is_buffering()
    }

    pub fn play(&self) -> anyhow::Result<()> {
        self.imp()
            .audio_player
            .try_set_state(PlaybackState::Playing)
    }

    pub fn pause(&self) -> anyhow::Result<()> {
        self.imp().audio_player.try_set_state(PlaybackState::Paused)
    }

    pub fn is_current_playing(&self, song: &Song) -> bool {
        self.song().as_ref() == Some(song)
    }

    fn set_playback_position_scale_value_blocking(&self, value: f64) {
        let imp = self.imp();
        let scale_handler_id = imp.scale_handler_id.get().unwrap();
        imp.playback_position_scale.block_signal(scale_handler_id);
        imp.playback_position_scale.set_value(value);
        imp.playback_position_scale.unblock_signal(scale_handler_id);
    }

    fn toggle_playback(&self) -> anyhow::Result<()> {
        let audio_player = &self.imp().audio_player;

        if audio_player.state() == PlaybackState::Playing {
            audio_player.try_set_state(PlaybackState::Paused)?;
        } else {
            audio_player.try_set_state(PlaybackState::Playing)?;
        }

        Ok(())
    }

    async fn update_duration_label(&self) -> anyhow::Result<()> {
        let imp = self.imp();

        let duration = imp.audio_player.duration().await?;

        let seconds = duration.as_secs_f64();
        imp.playback_position_scale.set_range(0.0, seconds);

        imp.duration_label.set_time(duration);

        Ok(())
    }

    fn update_playback_position_ui(&self) {
        let imp = self.imp();

        match imp.audio_player.query_position() {
            Ok(position) => {
                self.set_playback_position_scale_value_blocking(position.as_secs_f64());
                imp.playback_position_label.set_time(position);
            }
            Err(err) => {
                log::warn!("Error querying position: {:?}", err);
            }
        }
    }

    fn update_playback_state_ui(&self) {
        let imp = self.imp();
        let state = imp.audio_player.state();

        match state {
            PlaybackState::Stopped | PlaybackState::Loading => {
                self.set_playback_position_scale_value_blocking(0.0);
                imp.playback_position_label.reset();
                imp.playback_position_scale.set_sensitive(false);
            }
            PlaybackState::Playing | PlaybackState::Paused => {
                imp.playback_position_scale.set_sensitive(true);
            }
        }

        match state {
            PlaybackState::Stopped | PlaybackState::Paused | PlaybackState::Loading => {
                imp.playback_button
                    .set_icon_name("media-playback-start-symbolic");
            }
            PlaybackState::Playing => {
                imp.playback_button
                    .set_icon_name("media-playback-pause-symbolic");
            }
        }
    }

    fn update_album_art(&self) {
        self.imp().album_art.set_song(self.song());
    }

    fn update_actions_sensitivity(&self) {
        let has_song = self.song().is_some();
        self.imp().playback_position_scale.set_sensitive(has_song);
        self.action_set_enabled("audio-player-widget.toggle-playback", has_song);
        self.action_set_enabled("audio-player-widget.clear", has_song);
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
                    let imp = obj.imp();
                    imp.seek_timeout_id.replace(None);
                    if let Err(err) = imp.audio_player.seek(ClockTime::from_secs_f64(value)) {
                        log::warn!("Failed to seek to `{value}` secs: {err:?}");
                    }
                }),
            )));
    }

    fn setup_audio_player(&self) {
        let imp = self.imp();

        let scale_handler_id = imp.playback_position_scale.connect_value_changed(
            clone!(@weak self as obj => move |scale| {
                obj.on_playback_position_scale_value_changed(scale);
            }),
        );
        imp.scale_handler_id.set(scale_handler_id).unwrap();

        imp.audio_player
            .connect_state_notify(clone!(@weak self as obj => move |_| {
                obj.update_playback_state_ui();
            }));

        glib::timeout_add_local(
            Duration::from_millis(500),
            clone!(@weak self as obj => @default-return Continue(false), move || {
                if obj.imp().audio_player.state() == PlaybackState::Playing {
                    obj.update_playback_position_ui();
                }

                Continue(true)
            }),
        );
    }
}

impl Default for AudioPlayerWidget {
    fn default() -> Self {
        Self::new()
    }
}
