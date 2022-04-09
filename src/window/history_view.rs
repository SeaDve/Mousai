use adw::prelude::*;
use gettextrs::{gettext, ngettext};
use gtk::{
    glib::{self, clone},
    subclass::prelude::*,
};
use once_cell::unsync::OnceCell;

use std::cell::Cell;

use super::{song_cell::SongCell, song_page::SongPage, Window};
use crate::{
    model::{Song, SongList},
    song_player::SongPlayer,
};

mod imp {
    use super::*;
    use glib::WeakRef;
    use gtk::CompositeTemplate;
    use once_cell::sync::Lazy;

    #[derive(Debug, Default, CompositeTemplate)]
    #[template(resource = "/io/github/seadve/Mousai/ui/history-view.ui")]
    pub struct HistoryView {
        #[template_child]
        pub stack: TemplateChild<gtk::Stack>,
        #[template_child]
        pub history_child: TemplateChild<gtk::Box>,
        #[template_child]
        pub header_bar_stack: TemplateChild<gtk::Stack>,
        #[template_child]
        pub main_header_bar: TemplateChild<gtk::HeaderBar>,
        #[template_child]
        pub selection_mode_header_bar: TemplateChild<gtk::HeaderBar>,
        #[template_child]
        pub selection_mode_menu_button: TemplateChild<gtk::MenuButton>,
        #[template_child]
        pub selection_mode_bar: TemplateChild<gtk::ActionBar>,
        #[template_child]
        pub search_bar: TemplateChild<gtk::SearchBar>,
        #[template_child]
        pub search_entry: TemplateChild<gtk::SearchEntry>,
        #[template_child]
        pub history_stack: TemplateChild<gtk::Stack>,
        #[template_child]
        pub main_page: TemplateChild<gtk::ScrolledWindow>,
        #[template_child]
        pub grid: TemplateChild<gtk::GridView>,
        #[template_child]
        pub empty_page: TemplateChild<adw::StatusPage>,
        #[template_child]
        pub empty_search_page: TemplateChild<adw::StatusPage>,
        #[template_child]
        pub song_child: TemplateChild<SongPage>,

        pub is_selection_mode: Cell<bool>,
        pub song_list: OnceCell<WeakRef<SongList>>,
        pub filter_model: OnceCell<WeakRef<gtk::FilterListModel>>,
        pub selection_model: OnceCell<WeakRef<gtk::MultiSelection>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for HistoryView {
        const NAME: &'static str = "MsaiHistoryView";
        type Type = super::HistoryView;
        type ParentType = gtk::Widget;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();

            klass.install_action("history-view.toggle-selection-mode", None, |obj, _, _| {
                if obj.is_selection_mode() {
                    obj.set_selection_mode(false);
                } else {
                    obj.unselect_all();
                    obj.set_selection_mode(true);
                }
            });

            klass.install_action("history-view.select-all", None, |obj, _, _| {
                obj.select_all();
            });

            klass.install_action("history-view.select-none", None, |obj, _, _| {
                obj.unselect_all();
            });

            klass.install_action("history-view.remove-selected-songs", None, |obj, _, _| {
                if let Some(song_list) = obj
                    .imp()
                    .song_list
                    .get()
                    .and_then(|song_list| song_list.upgrade())
                {
                    obj.snapshot_selected_songs().iter().for_each(|song| {
                        song_list.remove(&song.id());
                    });
                }
            });
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for HistoryView {
        fn properties() -> &'static [glib::ParamSpec] {
            static PROPERTIES: Lazy<Vec<glib::ParamSpec>> = Lazy::new(|| {
                vec![glib::ParamSpecBoolean::new(
                    "is-selection-mode",
                    "Is Selection Mode",
                    "Whether self is on selection mode",
                    false,
                    glib::ParamFlags::READABLE,
                )]
            });
            PROPERTIES.as_ref()
        }

        fn property(&self, obj: &Self::Type, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
            match pspec.name() {
                "is-selection-mode" => obj.is_selection_mode().to_value(),
                _ => unimplemented!(),
            }
        }

        fn constructed(&self, obj: &Self::Type) {
            self.parent_constructed(obj);

            obj.add_css_class("view");

            obj.setup_grid();

            obj.update_selection_mode_menu_button();
            obj.update_remove_selected_songs_action();
            obj.update_selection_mode_ui();
            obj.show_history();
        }

        fn dispose(&self, obj: &Self::Type) {
            while let Some(child) = obj.first_child() {
                child.unparent();
            }
        }
    }

    impl WidgetImpl for HistoryView {}
}

glib::wrapper! {
    pub struct HistoryView(ObjectSubclass<imp::HistoryView>)
        @extends gtk::Widget;
}

impl HistoryView {
    pub fn new() -> Self {
        glib::Object::new(&[]).expect("Failed to create HistoryView")
    }

    pub fn connect_selection_mode_notify<F>(&self, f: F) -> glib::SignalHandlerId
    where
        F: Fn(&Self) + 'static,
    {
        self.connect_notify_local(Some("is-selection-mode"), move |obj, _| f(obj))
    }

    pub fn is_selection_mode(&self) -> bool {
        self.imp().is_selection_mode.get()
    }

    pub fn stop_selection_mode(&self) {
        self.set_selection_mode(false);
    }

    pub fn search_bar(&self) -> gtk::SearchBar {
        self.imp().search_bar.get()
    }

    pub fn is_on_song_page(&self) -> bool {
        let imp = self.imp();
        imp.stack.visible_child().as_ref() == Some(imp.song_child.upcast_ref())
    }

    pub fn show_history(&self) {
        let imp = self.imp();
        self.update_history_stack();
        imp.stack.set_visible_child(&imp.history_child.get());
    }

    pub fn show_song(&self, song: &Song) {
        let imp = self.imp();
        imp.song_child.set_song(Some(song.clone()));
        imp.stack.set_visible_child(&imp.song_child.get());
    }

    /// Must only be called once
    pub fn bind_player(&self, player: &SongPlayer) {
        self.imp().song_child.bind_player(player);
    }

    /// Must only be called once
    pub fn bind_song_list(&self, song_list: &SongList) {
        let imp = self.imp();

        song_list.connect_items_changed(clone!(@weak self as obj => move |_, _, _, _| {
            obj.update_history_stack();
        }));

        let filter = gtk::CustomFilter::new(
            clone!(@weak self as obj => @default-return false, move |item| {
                let search_text = obj.imp().search_entry.text().to_lowercase();
                let song = item.downcast_ref::<Song>().unwrap();
                song.title().to_lowercase().contains(&search_text) || song.artist().to_lowercase().contains(&search_text)
            }),
        );
        let filter_model = gtk::FilterListModel::new(Some(song_list), Some(&filter));
        filter_model.connect_items_changed(clone!(@weak self as obj => move |_, _, _, _| {
            obj.update_history_stack();
        }));

        imp.search_entry.connect_search_changed(
            clone!(@weak self as obj, @weak filter => move |_| {
                filter.changed(gtk::FilterChange::Different);
                obj.update_history_stack();
            }),
        );

        let sorter = gtk::CustomSorter::new(|item_1, item_2| {
            let song_1 = item_1.downcast_ref::<Song>().unwrap();
            let song_2 = item_2.downcast_ref::<Song>().unwrap();
            song_2.last_heard().cmp(&song_1.last_heard()).into()
        });
        let sort_model = gtk::SortListModel::new(Some(&filter_model), Some(&sorter));

        // FIXME save selection even when the song are filtered from FilterListModel
        let selection_model = gtk::MultiSelection::new(Some(&sort_model));
        selection_model.connect_selection_changed(clone!(@weak self as obj => move |_, _, _| {
            if obj.is_selection_mode() {
                obj.update_selection_mode_menu_button();
                obj.update_remove_selected_songs_action();
            }
        }));
        selection_model.connect_items_changed(clone!(@weak self as obj => move |_, _, _, _| {
            if obj.is_selection_mode() {
                obj.update_selection_mode_menu_button();
                obj.update_remove_selected_songs_action();
            }
        }));

        let grid = imp.grid.get();
        grid.set_model(Some(&selection_model));
        grid.connect_activate(
            clone!(@weak self as obj, @weak selection_model => move |_, index| {
                match selection_model.item(index).and_then(|song| song.downcast::<Song>().ok()) {
                    Some(ref song) => obj.show_song(song),
                    None => log::error!("Activated `{index}`, but found no song.")
                }
            }),
        );

        imp.song_list.set(song_list.downgrade()).unwrap();
        imp.filter_model.set(filter_model.downgrade()).unwrap();
        imp.selection_model
            .set(selection_model.downgrade())
            .unwrap();

        self.update_history_stack();
    }

    fn select_all(&self) {
        if let Some(selection_model) = self
            .imp()
            .selection_model
            .get()
            .and_then(|model| model.upgrade())
        {
            selection_model.select_all();
        }
    }

    fn unselect_all(&self) {
        if let Some(selection_model) = self
            .imp()
            .selection_model
            .get()
            .and_then(|model| model.upgrade())
        {
            selection_model.unselect_all();
        }
    }

    fn snapshot_selected_songs(&self) -> Vec<Song> {
        self.imp()
            .selection_model
            .get()
            .and_then(|model| model.upgrade())
            .map_or(Vec::new(), |selection_model| {
                let mut selected_songs = Vec::new();
                for position in 0..selection_model.n_items() {
                    if selection_model.is_selected(position) {
                        selected_songs.push(
                            selection_model
                                .item(position)
                                .unwrap()
                                .downcast::<Song>()
                                .unwrap(),
                        );
                    }
                }
                selected_songs
            })
    }

    fn set_selection_mode(&self, is_selection_mode: bool) {
        if is_selection_mode == self.is_selection_mode() {
            return;
        }

        self.imp().is_selection_mode.set(is_selection_mode);
        self.update_selection_mode_ui();

        self.notify("is-selection-mode");
    }

    fn update_selection_mode_ui(&self) {
        let imp = self.imp();

        if self.is_selection_mode() {
            imp.header_bar_stack
                .set_visible_child(&imp.selection_mode_header_bar.get());
            imp.selection_mode_bar.set_revealed(true);
        } else {
            imp.header_bar_stack
                .set_visible_child(&imp.main_header_bar.get());
            imp.selection_mode_bar.set_revealed(false);
        }
    }

    fn update_history_stack(&self) {
        let imp = self.imp();

        let search_text = imp.search_entry.text();

        if imp
            .filter_model
            .get()
            .and_then(|filter_model| filter_model.upgrade())
            .map_or(true, |filter_model| filter_model.n_items() == 0)
            && !search_text.is_empty()
        {
            imp.history_stack
                .set_visible_child(&imp.empty_search_page.get());
        } else if imp
            .song_list
            .get()
            .and_then(|song_list| song_list.upgrade())
            .map_or(true, |song_list| song_list.n_items() == 0)
            && search_text.is_empty()
        {
            imp.history_stack.set_visible_child(&imp.empty_page.get());
        } else {
            imp.history_stack.set_visible_child(&imp.main_page.get());
        }
    }

    fn update_selection_mode_menu_button(&self) {
        let imp = self.imp();
        let selection_size = imp
            .selection_model
            .get()
            .and_then(|model| model.upgrade())
            .map_or(0, |model| model.selection().size());

        let label = match selection_size {
            0 => gettext("Click on items to select them"),
            1.. => ngettext!(
                "Selected {} song",
                "Selected {} songs",
                selection_size as u32,
                selection_size
            ),
        };

        imp.selection_mode_menu_button.set_label(&label);
    }

    fn update_remove_selected_songs_action(&self) {
        let is_selection_empty = self
            .imp()
            .selection_model
            .get()
            .and_then(|model| model.upgrade())
            .map_or(true, |model| model.selection().is_empty());

        self.action_set_enabled("history-view.remove-selected-songs", !is_selection_empty);
    }

    fn setup_grid(&self) {
        let factory = gtk::SignalListItemFactory::new();
        factory.connect_setup(clone!(@weak self as obj => move |_, list_item| {
            let song_cell = SongCell::new();
            list_item
                .property_expression("item")
                .bind(&song_cell, "song", glib::Object::NONE);

            let check_button = gtk::CheckButton::builder()
                .css_classes(vec!["selection-mode".into()])
                .valign(gtk::Align::End)
                .halign(gtk::Align::End)
                .margin_end(12)
                .margin_bottom(12)
                .build();
            check_button.connect_active_notify(clone!(@weak obj, @weak list_item => move |check_button| {
                if let Some(selection_model) = obj.imp().selection_model.get().and_then(|model| model.upgrade()) {
                    if check_button.is_active() {
                        selection_model.select_item(list_item.position(), false);
                    } else {
                        selection_model.unselect_item(list_item.position());
                    }
                }
            }));
            obj.bind_property("is-selection-mode", &check_button, "visible")
                .flags(glib::BindingFlags::SYNC_CREATE)
                .build();
            list_item.property_expression("selected").bind(
                &check_button,
                "active",
                glib::Object::NONE,
            );

            let overlay = gtk::Overlay::builder().child(&song_cell).build();
            overlay.add_overlay(&check_button);
            list_item.set_child(Some(&overlay));
        }));
        factory.connect_bind(clone!(@weak self as obj => move |_, list_item| {
            let song_cell: SongCell = list_item
                .child()
                .and_then(|widget| widget.downcast::<gtk::Overlay>().ok())
                .and_then(|overlay| overlay.child())
                .and_then(|child| child.downcast().ok())
                .expect("HistoryView list item should have widget tree of gtk::Overlay > SongCell");

            if let Some(window) = obj.root().and_then(|root| root.downcast::<Window>().ok()) {
                song_cell.bind(Some(&window.player()));
            } else {
                log::error!("Cannot bind SongCell to AudioPlayerWidget: HistoryView doesn't have root");
            }
        }));
        factory.connect_unbind(|_, list_item| {
            let song_cell: SongCell = list_item
                .child()
                .and_then(|widget| widget.downcast::<gtk::Overlay>().ok())
                .and_then(|overlay| overlay.child())
                .and_then(|child| child.downcast().ok())
                .expect("HistoryView list item should have widget tree of gtk::Overlay > SongCell");

            song_cell.unbind();
        });

        let grid = self.imp().grid.get();
        grid.set_factory(Some(&factory));

        self.bind_property("is-selection-mode", &grid, "single-click-activate")
            .flags(glib::BindingFlags::SYNC_CREATE | glib::BindingFlags::INVERT_BOOLEAN)
            .build();
    }
}

impl Default for HistoryView {
    fn default() -> Self {
        Self::new()
    }
}
