use gettextrs::gettext;
use gtk::{
    glib::{self, clone},
    prelude::*,
    subclass::prelude::*,
};
use once_cell::unsync::OnceCell;

use super::{audio_visualizer::AudioVisualizer, Window};
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
        pub listen_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub stack: TemplateChild<gtk::Stack>,
        #[template_child]
        pub main_page: TemplateChild<gtk::ScrolledWindow>,
        #[template_child]
        pub empty_page: TemplateChild<adw::StatusPage>,
        #[template_child]
        pub listening_page: TemplateChild<gtk::Box>,
        #[template_child]
        pub recognizing_page: TemplateChild<adw::StatusPage>,
        #[template_child]
        pub visualizer: TemplateChild<AudioVisualizer>,

        pub history: OnceCell<SongList>,
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
                    self.history.set(SongList::default()).unwrap();
                }
            }

            obj.setup_history_view();
            obj.setup_recognizer();
            obj.setup_visualizer();

            obj.update_stack();
            obj.update_listen_button();
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

    fn show_error(&self, text: &str, secondary_text: &str) {
        let error_dialog = gtk::MessageDialog::builder()
            .text(text)
            .secondary_text(secondary_text)
            .buttons(gtk::ButtonsType::Ok)
            .message_type(gtk::MessageType::Error)
            .modal(true)
            .build();

        error_dialog.set_transient_for(
            self.root()
                .and_then(|root| root.downcast::<Window>().ok())
                .as_ref(),
        );
        error_dialog.connect_response(|error_dialog, _| error_dialog.destroy());
        error_dialog.present();
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
                if let Err(err) = imp.recognizer.listen() {
                    self.show_error(&gettext("Failed to start recording"), &err.to_string());
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
                    obj.history().append(song);
                }
                Err(err) => {
                    // TODO improve errors (more specific)
                    obj.show_error(&gettext("Something went wrong"), &err.to_string());
                    log::error!("Something went wrong: {:?} \n(dbg {:#?})", err, err);
                }
            }
        }));
    }

    fn update_listen_button(&self) {
        let imp = self.imp();

        match imp.recognizer.state() {
            RecognizerState::Null => {
                self.action_set_enabled("win.toggle-listen", true);

                imp.listen_button.remove_css_class("destructive-action");
                imp.listen_button.add_css_class("suggested-action");

                imp.listen_button.set_label(&gettext("Listen"));
                imp.listen_button
                    .set_tooltip_text(Some(&gettext("Start Identifying Music")));
            }
            RecognizerState::Listening => {
                self.action_set_enabled("win.toggle-listen", true);

                imp.listen_button.remove_css_class("suggested-action");
                imp.listen_button.add_css_class("destructive-action");

                imp.listen_button.set_label(&gettext("Cancel"));
                imp.listen_button
                    .set_tooltip_text(Some(&gettext("Cancel Listening")));
            }
            RecognizerState::Recognizing => {
                self.action_set_enabled("win.toggle-listen", false);
            }
        }
    }

    fn update_stack(&self) {
        let imp = self.imp();

        match imp.recognizer.state() {
            RecognizerState::Listening => {
                imp.stack.set_visible_child(&imp.listening_page.get());
                return;
            }
            RecognizerState::Recognizing => {
                imp.stack.set_visible_child(&imp.recognizing_page.get());
                return;
            }
            RecognizerState::Null => (),
        }

        if self.history().is_empty() {
            imp.stack.set_visible_child(&imp.empty_page.get());
        } else {
            imp.stack.set_visible_child(&imp.main_page.get());
        }
    }

    fn setup_history_view(&self) {
        let history = self.history();

        history.connect_items_changed(clone!(@weak self as obj => move |_, _, _, _| {
            obj.update_stack();
        }));

        let sorter = gtk::CustomSorter::new(move |obj1, obj2| {
            let song_1 = obj1.downcast_ref::<Song>().unwrap();
            let song_2 = obj2.downcast_ref::<Song>().unwrap();
            song_2.last_heard().cmp(&song_1.last_heard()).into()
        });
        let sort_model = gtk::SortListModel::new(Some(&history), Some(&sorter));

        let selection_model = gtk::NoSelection::new(Some(&sort_model));

        let history_view = self.imp().history_view.get();
        history_view.set_model(Some(&selection_model));
        history_view.connect_activate(clone!(@weak self as obj => move |_, index| {
            match selection_model.item(index).and_then(|song| song.downcast::<Song>().ok()) {
                Some(ref song) => obj.emit_by_name("song-activated", &[song]),
                None => log::warn!("Activated `{index}`, but found no song.")
            }
        }));
    }

    fn setup_recognizer(&self) {
        let imp = self.imp();

        imp.recognizer
            .connect_state_notify(clone!(@weak self as obj => move |_| {
                obj.update_stack();
                obj.update_listen_button();
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
