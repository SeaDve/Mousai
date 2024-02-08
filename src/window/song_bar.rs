use gtk::{
    glib::{self, clone, closure_local},
    prelude::*,
    subclass::prelude::*,
};

use std::{
    cell::{OnceCell, RefCell},
    time::Duration,
};

use super::{
    album_cover::AlbumCover,
    playback_button::{PlaybackButton, PlaybackButtonMode},
};
use crate::{
    player::{Player, PlayerState},
    song::Song,
};

mod imp {
    use glib::subclass::Signal;
    use once_cell::sync::Lazy;

    use super::*;

    #[derive(Default, gtk::CompositeTemplate)]
    #[template(resource = "/io/github/seadve/Mousai/ui/song-bar.ui")]
    pub struct SongBar {
        #[template_child]
        pub(super) center_box: TemplateChild<gtk::CenterBox>, // Unused
        #[template_child]
        pub(super) album_cover: TemplateChild<AlbumCover>,
        #[template_child]
        pub(super) title_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub(super) artist_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub(super) playback_button: TemplateChild<PlaybackButton>,
        #[template_child]
        pub(super) playback_position_scale: TemplateChild<gtk::Scale>,
        #[template_child]
        pub(super) playback_position_duration_label: TemplateChild<gtk::Label>,

        pub(super) scale_handler_id: OnceCell<glib::SignalHandlerId>,
        pub(super) seek_timeout_id: RefCell<Option<glib::SourceId>>,
        pub(super) player: OnceCell<Player>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for SongBar {
        const NAME: &'static str = "MsaiSongBar";
        type Type = super::SongBar;
        type ParentType = gtk::Widget;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();

            klass.set_accessible_role(gtk::AccessibleRole::Group);

            klass.install_action("song-bar.clear", None, |obj, _, _| {
                obj.player().set_song(Song::NONE);
            });

            klass.install_action("song-bar.activate", None, |obj, _, _| {
                if let Some(ref song) = obj.player().song() {
                    obj.emit_by_name::<()>("activated", &[song]);
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
                vec![Signal::builder("activated")
                    .param_types([Song::static_type()])
                    .build()]
            });

            SIGNALS.as_ref()
        }

        fn constructed(&self) {
            self.parent_constructed();

            let obj = self.obj();

            self.scale_handler_id
                .set(self.playback_position_scale.connect_value_changed(
                    clone!(@weak obj => move |scale| {
                        obj.on_playback_position_scale_value_changed(scale);
                    }),
                ))
                .unwrap();
        }

        fn dispose(&self) {
            self.dispose_template();
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
        glib::Object::new()
    }

    pub fn connect_activated<F>(&self, f: F) -> glib::SignalHandlerId
    where
        F: Fn(&Self, &Song) + 'static,
    {
        self.connect_closure(
            "activated",
            true,
            closure_local!(|obj: &Self, song: &Song| {
                f(obj, song);
            }),
        )
    }

    /// Must only be called once.
    pub fn bind_player(&self, player: &Player) {
        let imp = self.imp();

        imp.player.set(player.clone()).unwrap();

        player.connect_song_notify(clone!(@weak self as obj => move |_| {
            obj.update_song_ui();
        }));

        player.connect_state_notify(clone!(@weak self as obj => move |_| {
            obj.update_playback_button();
        }));

        player.connect_position_notify(clone!(@weak self as obj => move |_| {
            obj.update_playback_position();
            obj.update_playback_position_duration_label();
        }));

        player.connect_duration_notify(clone!(@weak self as obj => move |_| {
            obj.update_playback_position_scale_range();
            obj.update_playback_position_duration_label();
        }));

        self.update_song_ui();
        self.update_playback_button();
        self.update_playback_position();
        self.update_playback_position_scale_range();
        self.update_playback_position_duration_label();
    }

    fn player(&self) -> &Player {
        self.imp().player.get().expect("player must be bound")
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
                    obj.player().seek(gst::ClockTime::from_seconds_f64(value));
                }),
            )));
    }

    fn update_song_ui(&self) {
        let imp = self.imp();
        let song = self.player().song();

        imp.title_label
            .set_label(&song.as_ref().map(|s| s.title()).unwrap_or_default());
        imp.artist_label
            .set_label(&song.as_ref().map(|s| s.artist()).unwrap_or_default());

        let has_song = song.is_some();
        self.imp().playback_position_scale.set_sensitive(has_song);
        self.action_set_enabled("song-bar.clear", has_song);

        imp.album_cover.set_song(song.as_ref());
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

    fn update_playback_position(&self) {
        let position = self.player().position();
        self.set_playback_position_scale_value_blocking(position.seconds_f64());
    }

    fn update_playback_position_scale_range(&self) {
        let imp = self.imp();
        let duration = self.player().duration();
        imp.playback_position_scale
            .set_range(0.0, duration.seconds_f64());
    }

    fn update_playback_position_duration_label(&self) {
        let player = self.player();
        self.imp()
            .playback_position_duration_label
            .set_label(&format!(
                "{} / {}",
                format_clock_time_minute_sec(player.position()),
                format_clock_time_minute_sec(player.duration())
            ));
    }
}

impl Default for SongBar {
    fn default() -> Self {
        Self::new()
    }
}

/// Displays `gst::ClockTime` in a `MM∶SS` format with padding for SS.
pub fn format_clock_time_minute_sec(clock_time: gst::ClockTime) -> String {
    let seconds = clock_time.seconds();

    let minutes_display = seconds / 60;
    let seconds_display = seconds % 60;

    format!("{}∶{:02}", minutes_display, seconds_display)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_clock_time_minute_sec() {
        #[track_caller]
        fn test(clock_time: gst::ClockTime, string: &str) {
            assert_eq!(format_clock_time_minute_sec(clock_time), string);
        }

        test(gst::ClockTime::ZERO, "0∶00");
        test(gst::ClockTime::from_seconds(31), "0∶31");
        test(gst::ClockTime::from_seconds(59 * 60 + 59), "59∶59");

        test(gst::ClockTime::from_seconds(60 * 60), "60∶00");
        test(gst::ClockTime::from_seconds(100 * 60 + 20), "100∶20");
        test(gst::ClockTime::MAX, "307445734∶33");
    }
}
