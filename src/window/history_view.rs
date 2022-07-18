use adw::prelude::*;
use gettextrs::{gettext, ngettext};
use gtk::{
    gdk,
    glib::{self, clone, closure},
    subclass::prelude::*,
};
use once_cell::unsync::OnceCell;

use std::cell::{Cell, RefCell};

use super::{song_page::SongPage, song_tile::SongTile, Window};
use crate::{
    config::APP_ID,
    model::{Song, SongList},
    player::Player,
    Application,
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
        pub remove_selected_songs_button: TemplateChild<gtk::Button>,
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

        pub is_selection_mode: Cell<bool>,

        pub player: OnceCell<WeakRef<Player>>,
        pub song_list: OnceCell<WeakRef<SongList>>,
        pub filter_model: OnceCell<WeakRef<gtk::FilterListModel>>,
        pub selection_model: OnceCell<WeakRef<gtk::MultiSelection>>,

        pub removed_purgatory: RefCell<Vec<Song>>,
        pub undo_remove_toast: RefCell<Option<adw::Toast>>,

        pub song_pages: RefCell<Vec<(SongPage, glib::SignalHandlerId)>>,
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

            klass.install_action("history-view.copy-selected-song", None, |obj, _, _| {
                let selected_songs = obj.snapshot_selected_songs();

                if selected_songs.len() > 1 {
                    log::error!(
                        "Copying should not be allowed when there is more than one selected."
                    );
                }

                if let Some(song) = selected_songs.get(0) {
                    if let Some(display) = gdk::Display::default() {
                        display.clipboard().set_text(&format!(
                            "{} - {}",
                            song.artist(),
                            song.title()
                        ));

                        let toast = adw::Toast::new(&gettext("Copied song to clipboard"));
                        Application::default().add_toast(&toast);
                    }
                } else {
                    log::error!("Failed to copy song: There is no selected song");
                }
            });

            klass.install_action("history-view.remove-selected-songs", None, |obj, _, _| {
                obj.snapshot_selected_songs()
                    .iter()
                    .for_each(|selected_song| {
                        obj.remove_song(selected_song);
                    });
                obj.show_undo_remove_toast();
            });
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for HistoryView {
        fn properties() -> &'static [glib::ParamSpec] {
            static PROPERTIES: Lazy<Vec<glib::ParamSpec>> = Lazy::new(|| {
                vec![
                    // Whether self is on selection mode
                    glib::ParamSpecBoolean::builder("is-selection-mode")
                        .flags(glib::ParamFlags::READABLE)
                        .build(),
                ]
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

            self.empty_page.set_icon_name(Some(APP_ID));
            obj.setup_grid();

            obj.update_selection_actions();
            obj.update_selection_mode_ui();

            obj.update_history_stack();
            self.stack.set_visible_child(&self.history_child.get());
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
        imp.stack
            .visible_child()
            .and_then(|child| child.downcast::<SongPage>().ok())
            .is_some()
    }

    pub fn pop_song_page(&self) {
        let imp = self.imp();

        let song_page_item = imp.song_pages.borrow_mut().pop();
        if let Some(item) = song_page_item {
            self.stack_remove_song_page_item(item);
        } else {
            self.update_history_stack();

            if imp.stack.visible_child().as_ref() != Some(&imp.history_child.get().upcast()) {
                log::error!(
                    "Popped all song pages, but the history child is still not the visible child"
                );
            }
        }
    }

    pub fn push_song_page(&self, song: &Song) {
        let imp = self.imp();

        // Return if the last SongPage's song is the same as the `song` argument.
        if let Some((song_page, _)) = imp.song_pages.borrow().last() {
            if Some(song.id()) == song_page.song().map(|song| song.id()) {
                return;
            }
        }

        let song_page = SongPage::new();
        if let Some(ref player) = imp.player.get().and_then(|player| player.upgrade()) {
            song_page.bind_player(player);
        }
        song_page.set_song(Some(song.clone()));
        let song_removed_handler_id =
            song_page.connect_song_removed(clone!(@weak self as obj => move |_, song| {
                obj.remove_song(song);
                obj.show_undo_remove_toast();
            }));

        imp.stack.add_child(&song_page);
        imp.stack.set_visible_child(&song_page);

        imp.song_pages
            .borrow_mut()
            .push((song_page, song_removed_handler_id));

        // User is already aware of the newly recognized song, so unset it.
        song.set_is_newly_recognized(false);
    }

    /// Must only be called once
    pub fn bind_player(&self, player: &Player) {
        self.imp().player.set(player.downgrade()).unwrap();
    }

    /// Must only be called once
    pub fn bind_song_list(&self, song_list: &SongList) {
        let imp = self.imp();

        song_list.connect_items_changed(clone!(@weak self as obj => move |_, _, _, _| {
            obj.update_history_stack();
        }));

        let filter = gtk::StringFilter::builder()
            .expression(
                &gtk::ClosureExpression::new::<String, &[gtk::Expression], _>(
                    &[],
                    closure!(|song: Song| [song.title(), song.artist()].join("")),
                ),
            )
            .match_mode(gtk::StringFilterMatchMode::Substring)
            .ignore_case(true)
            .build();

        let filter_model = gtk::FilterListModel::new(Some(song_list), Some(&filter));
        filter_model.connect_items_changed(clone!(@weak self as obj => move |_, _, _, _| {
            obj.update_history_stack();
        }));

        imp.search_entry.connect_search_changed(
            clone!(@weak self as obj, @weak filter => move |search_entry| {
                filter.set_search(Some(&search_entry.text()));
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
                obj.update_selection_actions();
            }
        }));
        selection_model.connect_items_changed(clone!(@weak self as obj => move |_, _, _, _| {
            if obj.is_selection_mode() {
                obj.update_selection_actions();
            }
        }));

        let grid = imp.grid.get();
        grid.set_model(Some(&selection_model));
        grid.connect_activate(
            clone!(@weak self as obj, @weak selection_model => move |_, index| {
                match selection_model.item(index).and_then(|song| song.downcast::<Song>().ok()) {
                    Some(ref song) => obj.push_song_page(song),
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

    pub fn undo_remove(&self) {
        let imp = self.imp();

        if let Some(song_list) = imp
            .song_list
            .get()
            .and_then(|song_list| song_list.upgrade())
        {
            song_list.append_many(imp.removed_purgatory.take());
        }
    }

    pub fn scroll_to_top(&self) {
        let item_position = 0_u32.to_variant();
        self.imp()
            .grid
            .activate_action("list.scroll-to-item", Some(&item_position))
            .unwrap();
    }

    fn stack_remove_song_page_item(&self, item: (SongPage, glib::SignalHandlerId)) {
        let (song_page, handler_id) = item;

        let imp = self.imp();

        imp.stack
            .set_visible_child(&imp.song_pages.borrow().last().map_or_else(
                || imp.history_child.get().upcast::<gtk::Widget>(),
                |(song_page, _)| song_page.clone().upcast::<gtk::Widget>(),
            ));
        imp.stack.remove(&song_page);

        song_page.disconnect(handler_id);
        song_page.unbind_player();
    }

    /// Adds song to purgatory, and remove any active `SongPage`s that contain it.
    fn remove_song(&self, song: &Song) {
        let imp = self.imp();

        if let Some(song_list) = imp
            .song_list
            .get()
            .and_then(|song_list| song_list.upgrade())
        {
            if let Some(removed_song) = song_list.remove(&song.id()) {
                imp.removed_purgatory.borrow_mut().push(removed_song);
            }
        } else {
            log::warn!("Failed to remove song: SongList not found");
        }

        // Since the song is removed from history, the SongPage that
        // contains it is dangling, so remove it.
        let song_page_index_to_rm = imp.song_pages.borrow().iter().position(|(song_page, _)| {
            song_page.song().map(|song_page_song| song_page_song.id()) == Some(song.id())
        });
        if let Some(index) = song_page_index_to_rm {
            let song_page_item = imp.song_pages.borrow_mut().remove(index);
            self.stack_remove_song_page_item(song_page_item);
        }
    }

    fn snapshot_selected_songs(&self) -> Vec<Song> {
        self.imp()
            .selection_model
            .get()
            .and_then(|model| model.upgrade())
            .map_or(Vec::new(), |selection_model| {
                (0..selection_model.n_items())
                    .filter(|position| selection_model.is_selected(*position))
                    .map(|position| {
                        selection_model
                            .item(position)
                            .unwrap()
                            .downcast::<Song>()
                            .unwrap()
                    })
                    .collect::<Vec<_>>()
            })
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

    fn set_selection_mode(&self, is_selection_mode: bool) {
        if is_selection_mode == self.is_selection_mode() {
            return;
        }

        self.imp().is_selection_mode.set(is_selection_mode);
        self.update_selection_mode_ui();
        self.update_selection_actions();

        self.notify("is-selection-mode");
    }

    fn show_undo_remove_toast(&self) {
        let imp = self.imp();

        if imp.undo_remove_toast.borrow().is_none() {
            let toast = adw::Toast::builder()
                .priority(adw::ToastPriority::High)
                .button_label(&gettext("_Undo"))
                .action_name("undo-remove-toast.dismiss")
                .build();

            toast.connect_dismissed(clone!(@weak self as obj => move |_| {
                let imp = obj.imp();
                imp.removed_purgatory.borrow_mut().clear();
                imp.undo_remove_toast.take();
            }));

            Application::default().add_toast(&toast);

            imp.undo_remove_toast.replace(Some(toast));
        }

        // Add this point we should already have a toast setup
        if let Some(ref toast) = *imp.undo_remove_toast.borrow() {
            let n_removed = imp.removed_purgatory.borrow().len();
            toast.set_title(&ngettext!(
                "Removed {} song",
                "Removed {} songs",
                n_removed as u32,
                n_removed
            ));
        }
    }

    fn update_selection_mode_ui(&self) {
        let imp = self.imp();
        let is_selection_mode = self.is_selection_mode();

        if is_selection_mode {
            imp.header_bar_stack
                .set_visible_child(&imp.selection_mode_header_bar.get());
            imp.grid.grab_focus();
        } else {
            imp.header_bar_stack
                .set_visible_child(&imp.main_header_bar.get());
        }

        imp.selection_mode_bar.set_revealed(is_selection_mode);
        imp.grid.set_enable_rubberband(is_selection_mode);
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

    fn update_selection_actions(&self) {
        let imp = self.imp();
        let selection_size = imp
            .selection_model
            .get()
            .and_then(|model| model.upgrade())
            .map_or(0, |model| model.selection().size());

        self.action_set_enabled("history-view.copy-selected-song", selection_size == 1);
        self.action_set_enabled("history-view.remove-selected-songs", selection_size != 0);

        imp.selection_mode_menu_button
            .set_label(&match selection_size {
                0 => gettext("Click on items to select them"),
                1.. => ngettext!(
                    "Selected {} song",
                    "Selected {} songs",
                    selection_size as u32,
                    selection_size
                ),
            });

        imp.remove_selected_songs_button
            .set_tooltip_text(Some(&ngettext(
                "Remove Song From History",
                "Remove Songs From History",
                selection_size as u32,
            )));
    }

    fn setup_grid(&self) {
        let factory = gtk::SignalListItemFactory::new();
        factory.connect_setup(clone!(@weak self as obj => move |_, list_item| {
            let song_tile = SongTile::new();
            if let Some(window) = obj.root().and_then(|root| root.downcast::<Window>().ok()) {
                song_tile.bind_player(&window.player());
            } else {
                log::error!("Cannot bind SongTile to Player: HistoryView doesn't have root");
            }
            list_item
                .property_expression("item")
                .bind(&song_tile, "song", glib::Object::NONE);

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

            let overlay = gtk::Overlay::builder().child(&song_tile).build();
            overlay.add_overlay(&check_button);
            list_item.set_child(Some(&overlay));
        }));

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
