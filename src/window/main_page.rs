use adw::prelude::*;
use gettextrs::gettext;
use gtk::{
    glib::{self, clone},
    subclass::prelude::*,
};
use once_cell::unsync::OnceCell;

use super::{audio_visualizer::AudioVisualizer, song_cell::SongCell, Window};
use crate::{
    model::{Song, SongList},
    recognizer::{Recognizer, RecognizerState},
    spawn, Application,
};

mod imp {
    use super::*;
    use glib::subclass::Signal;
    use gtk::CompositeTemplate;
    use once_cell::sync::Lazy;

    #[derive(Debug, Default, CompositeTemplate)]
    #[template(resource = "/io/github/seadve/Mousai/ui/main-page.ui")]
    pub struct MainPage {
        #[template_child]
        pub history_view: TemplateChild<gtk::GridView>,
        #[template_child]
        pub search_entry: TemplateChild<gtk::SearchEntry>,
        #[template_child]
        pub stack: TemplateChild<gtk::Stack>,
        #[template_child]
        pub main_page: TemplateChild<gtk::Box>,
        #[template_child]
        pub empty_page: TemplateChild<gtk::Box>,
        #[template_child]
        pub busy_page: TemplateChild<gtk::Box>,
        #[template_child]
        pub busy_page_title: TemplateChild<gtk::Label>,
        #[template_child]
        pub visualizer: TemplateChild<AudioVisualizer>,

        pub history: OnceCell<SongList>,
        pub recognizing_animation: OnceCell<adw::TimedAnimation>,
        pub recognizer: Recognizer,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for MainPage {
        const NAME: &'static str = "MsaiMainPage";
        type Type = super::MainPage;
        type ParentType = gtk::Widget;

        fn class_init(klass: &mut Self::Class) {
            Self::bind_template(klass);

            klass.install_action("main-page.toggle-listen", None, |obj, _, _| {
                obj.on_toggle_listen();
            });
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for MainPage {
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

        fn constructed(&self, obj: &Self::Type) {
            self.parent_constructed(obj);

            obj.add_css_class("view");

            match obj.load_history() {
                Ok(history) => self.history.set(history).unwrap(),
                Err(err) => {
                    log::error!("Failed to load history: {:?}", err);
                    obj.show_error(&gettext("Failed to load history"));
                    self.history.set(SongList::default()).unwrap();
                }
            }

            obj.setup_history_view();
            obj.setup_recognizer();
            obj.setup_visualizer();

            obj.update_stack();
            obj.update_toggle_listen_action();
        }

        fn dispose(&self, obj: &Self::Type) {
            while let Some(child) = obj.first_child() {
                child.unparent();
            }
        }
    }

    impl WidgetImpl for MainPage {}
}

glib::wrapper! {
    pub struct MainPage(ObjectSubclass<imp::MainPage>)
        @extends gtk::Widget;
}

impl MainPage {
    pub fn new() -> Self {
        glib::Object::new(&[]).expect("Failed to create MainPage")
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

    pub fn save_history(&self) -> anyhow::Result<()> {
        let json_str = serde_json::to_string(&self.history())?;
        let settings = Application::default().settings();
        settings.set_string("history", &json_str)?;
        Ok(())
    }

    fn load_history(&self) -> anyhow::Result<SongList> {
        let settings = Application::default().settings();
        let json_str = settings.string("history");
        let history: SongList = serde_json::from_str(&json_str)?;
        Ok(history)
    }

    fn show_error(&self, message: &str) {
        if let Some(window) = Application::default().main_window() {
            window.show_error(message);
        } else {
            log::warn!("Failed to show error: Found no MainWindow");
        }
    }

    fn stop_player(&self) -> anyhow::Result<()> {
        if let Some(player) = self
            .root()
            .and_then(|root| root.downcast::<Window>().ok())
            .map(|window| window.player())
        {
            player.set_song(None)?;
            Ok(())
        } else {
            Err(anyhow::anyhow!(
                "Failed stop audio player: AudioPlayerWidget was not found"
            ))
        }
    }

    fn history(&self) -> SongList {
        self.imp().history.get().unwrap().clone()
    }

    fn on_toggle_listen(&self) {
        let imp = self.imp();

        match imp.recognizer.state() {
            RecognizerState::Listening => {
                spawn!(clone!(@weak imp.recognizer as recognizer => async move {
                    recognizer.cancel().await;
                    log::info!("Cancelled recognizing");
                }));
            }
            RecognizerState::Null => {
                if let Err(err) = self.stop_player() {
                    log::warn!("Failed to stop player before listening: {err:?}");
                }

                if let Err(err) = imp.recognizer.listen() {
                    self.show_error(&gettext!("Failed to start recording: {}", err));
                    log::error!("Failed to start recording: {:?} \n(dbg {:#?})", err, err);
                }
            }
            RecognizerState::Recognizing => (),
        }
    }

    fn on_listen_done(&self, recognizer: &Recognizer) {
        spawn!(clone!(@weak recognizer, @weak self as obj => async move {
            log::info!("Listen done");

            match recognizer.listen_finish().await {
                Ok(song) => {
                    obj.emit_by_name::<()>("song-activated", &[&song]);
                    obj.history().append(song);
                }
                Err(err) => {
                    // TODO improve errors (more specific)
                    log::error!("Something went wrong: {:?} \n(dbg {:#?})", err, err);
                    obj.show_error(&gettext!("Something went wrong: {}", err));
                }
            }
        }));
    }

    fn update_toggle_listen_action(&self) {
        match self.imp().recognizer.state() {
            RecognizerState::Null | RecognizerState::Listening => {
                self.action_set_enabled("main-page.toggle-listen", true);
            }
            RecognizerState::Recognizing => {
                self.action_set_enabled("main-page.toggle-listen", false);
            }
        }
    }

    fn update_stack(&self) {
        let imp = self.imp();

        match imp.recognizer.state() {
            RecognizerState::Listening => {
                if let Some(recognizing_animation) = imp.recognizing_animation.get() {
                    imp.visualizer.clear_peaks();
                    recognizing_animation.pause();
                }

                imp.busy_page_title.set_label(&gettext("Listening…"));
                imp.stack.set_visible_child(&imp.busy_page.get());
                return;
            }
            RecognizerState::Recognizing => {
                let animation = imp.recognizing_animation.get_or_init(|| {
                    adw::TimedAnimation::builder()
                        .widget(&imp.visualizer.get())
                        .value_from(0.0)
                        .value_to(0.6)
                        .duration(1500)
                        .target(&adw::CallbackAnimationTarget::new(Some(Box::new(
                            clone!(@weak self as obj => move |value| {
                                obj.imp().visualizer.push_peak(value);
                            }),
                        ))))
                        .easing(adw::Easing::EaseOutExpo)
                        .repeat_count(u32::MAX)
                        .alternate(true)
                        .build()
                });
                imp.visualizer.clear_peaks();
                animation.play();

                imp.busy_page_title.set_label(&gettext("Recognizing…"));
                imp.stack.set_visible_child(&imp.busy_page.get());
                return;
            }
            RecognizerState::Null => {
                if let Some(recognizing_animation) = imp.recognizing_animation.get() {
                    imp.visualizer.clear_peaks();
                    recognizing_animation.pause();
                }
            }
        }

        if self.history().is_empty() {
            imp.stack.set_visible_child(&imp.empty_page.get());
        } else {
            imp.stack.set_visible_child(&imp.main_page.get());
        }
    }

    fn setup_history_view(&self) {
        let imp = self.imp();
        let history = self.history();

        history.connect_items_changed(clone!(@weak self as obj => move |_, _, _, _| {
            obj.update_stack();
        }));

        let filter = gtk::CustomFilter::new(
            clone!(@weak self as obj => @default-return false, move |item| {
                let search_text = obj.imp().search_entry.text().to_lowercase();
                let song = item.downcast_ref::<Song>().unwrap();
                song.title().to_lowercase().contains(&search_text) || song.artist().to_lowercase().contains(&search_text)
            }),
        );
        let filter_model = gtk::FilterListModel::new(Some(&history), Some(&filter));

        imp.search_entry
            .connect_search_changed(clone!(@weak filter => move |_| {
                filter.changed(gtk::FilterChange::Different);
            }));

        let sorter = gtk::CustomSorter::new(|item_1, item_2| {
            let song_1 = item_1.downcast_ref::<Song>().unwrap();
            let song_2 = item_2.downcast_ref::<Song>().unwrap();
            song_2.last_heard().cmp(&song_1.last_heard()).into()
        });
        let sort_model = gtk::SortListModel::new(Some(&filter_model), Some(&sorter));

        let selection_model = gtk::NoSelection::new(Some(&sort_model));

        let history_view = imp.history_view.get();

        let factory = gtk::SignalListItemFactory::new();
        factory.connect_setup(|_, list_item| {
            let song_cell = SongCell::new();

            list_item
                .property_expression("item")
                .bind(&song_cell, "song", glib::Object::NONE);

            list_item.set_child(Some(&song_cell));
        });
        factory.connect_bind(clone!(@weak self as obj => move |_, list_item| {
            let song_cell: SongCell = list_item
                .child()
                .and_then(|widget| widget.downcast().ok())
                .expect("HistoryView list item should have a child of SongCell");

            if let Some(window) = obj.root().and_then(|root| root.downcast::<Window>().ok()) {
                song_cell.bind(Some(&window.player()));
            } else {
                log::error!("Cannot bind SongCell to AudioPlayerWidget: MainPage doesn't have root");
            }
        }));
        factory.connect_unbind(|_, list_item| {
            let song_cell: SongCell = list_item
                .child()
                .and_then(|widget| widget.downcast().ok())
                .expect("HistoryView list item should have a child of SongCell");
            song_cell.unbind();
        });

        history_view.set_factory(Some(&factory));
        history_view.set_model(Some(&selection_model));

        history_view.connect_activate(clone!(@weak self as obj => move |_, index| {
            match selection_model.item(index).and_then(|song| song.downcast::<Song>().ok()) {
                Some(ref song) => obj.emit_by_name::<()>("song-activated", &[song]),
                None => log::error!("Activated `{index}`, but found no song.")
            }
        }));
    }

    fn setup_recognizer(&self) {
        let imp = self.imp();

        imp.recognizer
            .connect_state_notify(clone!(@weak self as obj => move |_| {
                obj.update_stack();
                obj.update_toggle_listen_action();
            }));

        imp.recognizer
            .connect_listen_done(clone!(@weak self as obj => move |recognizer| {
                obj.on_listen_done(recognizer);
            }));
    }

    fn setup_visualizer(&self) {
        let recorder = self.imp().recognizer.audio_recorder();

        recorder.connect_peak_notify(clone!(@weak self as obj => move |recorder| {
            let peak = 10_f64.powf(recorder.peak() / 20.0);
            obj.imp().visualizer.push_peak(peak as f64);
        }));

        recorder.connect_stopped(clone!(@weak self as obj => move |_| {
            obj.imp().visualizer.clear_peaks();
        }));
    }
}

impl Default for MainPage {
    fn default() -> Self {
        Self::new()
    }
}
