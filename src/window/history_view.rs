use adw::prelude::*;
use gettextrs::{gettext, ngettext};
use gtk::{
    gdk,
    glib::{self, clone, closure},
    subclass::prelude::*,
};
use once_cell::unsync::OnceCell;

use std::cell::{Cell, RefCell};

use super::{song_page::SongPage, song_tile::SongTile, AdaptiveMode};
use crate::{
    config::APP_ID,
    model::{FuzzyFilter, FuzzySorter, Song, SongList},
    player::Player,
    utils,
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
        pub(super) stack: TemplateChild<gtk::Stack>,
        #[template_child]
        pub(super) history_child: TemplateChild<gtk::Box>,
        #[template_child]
        pub(super) header_bar_stack: TemplateChild<gtk::Stack>,
        #[template_child]
        pub(super) main_header_bar: TemplateChild<gtk::HeaderBar>,
        #[template_child]
        pub(super) selection_mode_header_bar: TemplateChild<gtk::HeaderBar>,
        #[template_child]
        pub(super) selection_mode_menu_button: TemplateChild<gtk::MenuButton>,
        #[template_child]
        pub(super) selection_mode_bar: TemplateChild<gtk::ActionBar>,
        #[template_child]
        pub(super) remove_selected_songs_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub(super) search_bar: TemplateChild<gtk::SearchBar>,
        #[template_child]
        pub(super) search_entry: TemplateChild<gtk::SearchEntry>,
        #[template_child]
        pub(super) history_stack: TemplateChild<gtk::Stack>,
        #[template_child]
        pub(super) main_page: TemplateChild<gtk::ScrolledWindow>,
        #[template_child]
        pub(super) grid: TemplateChild<gtk::GridView>,
        #[template_child]
        pub(super) empty_page: TemplateChild<adw::StatusPage>,
        #[template_child]
        pub(super) empty_search_page: TemplateChild<adw::StatusPage>,

        pub(super) is_selection_mode: Cell<bool>,
        pub(super) adaptive_mode: Cell<AdaptiveMode>,

        pub(super) player: OnceCell<WeakRef<Player>>,
        pub(super) song_list: OnceCell<WeakRef<SongList>>,
        pub(super) filter_model: OnceCell<WeakRef<gtk::FilterListModel>>,
        pub(super) selection_model: OnceCell<WeakRef<gtk::MultiSelection>>,

        pub(super) removed_purgatory: RefCell<Vec<Song>>,
        pub(super) undo_remove_toast: RefCell<Option<adw::Toast>>,

        pub(super) song_pages: RefCell<Vec<(SongPage, glib::SignalHandlerId, glib::Binding)>>,
        pub(super) pending_stack_remove_song_page: RefCell<Option<SongPage>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for HistoryView {
        const NAME: &'static str = "MsaiHistoryView";
        type Type = super::HistoryView;
        type ParentType = gtk::Widget;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();

            klass.install_action("history-view.toggle-selection-mode", None, |obj, _, _| {
                // I don't know why exactly getting `is_selection_mode` first
                // before unselecting all, but it prevents flickering when cancelling
                // selection mode; probably, because we also set selection mode
                // on selection change callback.
                let is_selection_mode = obj.is_selection_mode();
                obj.unselect_all();
                obj.set_selection_mode(!is_selection_mode);
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
                    tracing::error!(
                        "Copying should not be allowed when there is more than one selected."
                    );
                }

                if let Some(song) = selected_songs.first() {
                    if let Some(display) = gdk::Display::default() {
                        display.clipboard().set_text(&format!(
                            "{} - {}",
                            song.artist(),
                            song.title()
                        ));

                        let toast = adw::Toast::new(&gettext("Copied song to clipboard"));
                        utils::app_instance().add_toast(&toast);
                    }
                } else {
                    tracing::error!("Failed to copy song: There is no selected song");
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
                    // Current adapative mode
                    glib::ParamSpecEnum::builder("adaptive-mode", AdaptiveMode::static_type())
                        .default_value(AdaptiveMode::default() as i32)
                        .flags(glib::ParamFlags::READWRITE | glib::ParamFlags::EXPLICIT_NOTIFY)
                        .build(),
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
                "adaptive-mode" => {
                    let adaptive_mode = value.get().unwrap();
                    obj.set_adaptive_mode(adaptive_mode);
                }
                _ => unimplemented!(),
            }
        }

        fn property(&self, obj: &Self::Type, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
            match pspec.name() {
                "is-selection-mode" => obj.is_selection_mode().to_value(),
                "adaptive-mode" => obj.adaptive_mode().to_value(),
                _ => unimplemented!(),
            }
        }

        fn constructed(&self, obj: &Self::Type) {
            self.parent_constructed(obj);

            self.stack
                .connect_transition_running_notify(clone!(@weak obj => move |stack| {
                    let imp = obj.imp();
                    if !stack.is_transition_running() {
                        if let Some(song_page) = imp.pending_stack_remove_song_page.take() {
                            stack.remove(&song_page);
                        }
                    }
                }));

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

    pub fn set_adaptive_mode(&self, adaptive_mode: AdaptiveMode) {
        if adaptive_mode == self.adaptive_mode() {
            return;
        }

        self.imp().adaptive_mode.set(adaptive_mode);
        self.notify("adaptive-mode");
    }

    pub fn adaptive_mode(&self) -> AdaptiveMode {
        self.imp().adaptive_mode.get()
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

    pub fn push_song_page(&self, song: &Song) {
        let imp = self.imp();

        // Return if the last `SongPage`s song has the same id as the given song
        if let Some((song_page, ..)) = imp.song_pages.borrow().last() {
            if Some(song.id()) == song_page.song().map(|song| song.id()) {
                return;
            }
        }

        let song_page = SongPage::new();
        song_page.bind_player(&self.player());
        song_page.set_song(Some(song.clone()));
        let song_removed_handler_id =
            song_page.connect_song_removed(clone!(@weak self as obj => move |_, song| {
                obj.remove_song(song);
                obj.show_undo_remove_toast();
            }));
        let adaptive_mode_binding = self
            .bind_property("adaptive-mode", &song_page, "adaptive-mode")
            .flags(glib::BindingFlags::SYNC_CREATE)
            .build();

        imp.stack.add_child(&song_page);
        imp.stack.set_visible_child(&song_page);

        imp.song_pages.borrow_mut().push((
            song_page,
            song_removed_handler_id,
            adaptive_mode_binding,
        ));

        // User is already aware of the newly recognized song, so unset it.
        song.set_is_newly_recognized(false);
    }

    pub fn pop_song_page(&self) {
        let imp = self.imp();

        let song_page_item = imp.song_pages.borrow_mut().pop();
        if let Some(item) = song_page_item {
            self.stack_remove_song_page_item(item);
        } else {
            self.update_history_stack();

            if imp.stack.visible_child().as_ref() != Some(&imp.history_child.get().upcast()) {
                tracing::error!(
                    "Popped all song pages, but the history child is still not the visible child"
                );
            }
        }
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

        let filter = FuzzyFilter::new();
        let sorter = FuzzySorter::new();

        let filter_model = gtk::FilterListModel::new(Some(song_list), Some(&filter));
        filter_model.connect_items_changed(clone!(@weak self as obj => move |_, _, _, _| {
            obj.update_history_stack();
        }));

        imp.search_entry.connect_search_changed(
            clone!(@weak self as obj, @weak filter, @weak sorter => move |search_entry| {
                let text = search_entry.text();
                filter.set_search(&text);
                sorter.set_search(&text);
                obj.update_history_stack();
            }),
        );

        let sort_model = gtk::SortListModel::new(Some(&filter_model), Some(&sorter));

        // FIXME save selection even when the song are filtered from FilterListModel
        let selection_model = gtk::MultiSelection::new(Some(&sort_model));
        selection_model.connect_selection_changed(clone!(@weak self as obj => move |model, _, _| {
            if obj.is_selection_mode() {
                if model.selection().size() == 0 {
                    obj.set_selection_mode(false);
                }

                obj.update_selection_actions();
            }
        }));
        selection_model.connect_items_changed(clone!(@weak self as obj => move |model, _, _, _| {
            if obj.is_selection_mode() {
                if model.selection().size() == 0 {
                    obj.set_selection_mode(false);
                }

                obj.update_selection_actions();
            }
        }));

        let grid = imp.grid.get();
        grid.set_model(Some(&selection_model));
        grid.connect_activate(
            clone!(@weak self as obj, @weak selection_model => move |_, index| {
                match selection_model.item(index).and_then(|song| song.downcast::<Song>().ok()) {
                    Some(ref song) => obj.push_song_page(song),
                    None => tracing::error!("Activated `{index}`, but found no song.")
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

    fn player(&self) -> Player {
        self.imp()
            .player
            .get()
            .expect("Player was not bound on HistoryView")
            .upgrade()
            .expect("Player was dropped")
    }

    fn stack_remove_song_page_item(&self, item: (SongPage, glib::SignalHandlerId, glib::Binding)) {
        let (song_page, handler_id, binding) = item;

        let imp = self.imp();

        imp.stack
            .set_visible_child(&imp.song_pages.borrow().last().map_or_else(
                || imp.history_child.get().upcast::<gtk::Widget>(),
                |(song_page, ..)| song_page.clone().upcast::<gtk::Widget>(),
            ));

        song_page.disconnect(handler_id);
        song_page.unbind_player();

        binding.unbind();

        imp.pending_stack_remove_song_page.replace(Some(song_page));
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
            tracing::warn!("Failed to remove song: SongList not found");
        }

        // Since the song is removed from history, the `SongPage`s that
        // contain it is dangling, so remove them.
        let (drained, song_pages) = imp
            .song_pages
            .take()
            .into_iter()
            // FIXME use Vec::drain_filter
            .partition(|(song_page, ..)| {
                song_page.song().map(|song_page_song| song_page_song.id()) == Some(song.id())
            });
        imp.song_pages.replace(song_pages);

        for item in drained {
            self.stack_remove_song_page_item(item);
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
                .action_name("undo-remove-toast.undo")
                .build();

            toast.connect_dismissed(clone!(@weak self as obj => move |_| {
                let imp = obj.imp();
                imp.removed_purgatory.borrow_mut().clear();
                imp.undo_remove_toast.take();
            }));

            utils::app_instance().add_toast(&toast);

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
        imp.grid.set_single_click_activate(!is_selection_mode);
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
                0 => gettext("Select items"),
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
            song_tile.bind_player(&obj.player());

            obj.bind_property("is-selection-mode", &song_tile, "is-selection-mode")
                .flags(glib::BindingFlags::SYNC_CREATE)
                .build();
            obj.bind_property("adaptive-mode", &song_tile, "adaptive-mode")
                .flags(glib::BindingFlags::SYNC_CREATE)
                .build();

            song_tile.connect_active_notify(clone!(@weak obj, @weak list_item => move |tile| {
                if let Some(selection_model) = obj.imp().selection_model.get().and_then(|model| model.upgrade()) {
                    if tile.is_active() {
                        selection_model.select_item(list_item.position(), false);
                    } else {
                        selection_model.unselect_item(list_item.position());
                    }
                }
            }));
            song_tile.connect_request_selection_mode(clone!(@weak obj => move |_| {
                obj.set_selection_mode(true);
            }));

            list_item
                .property_expression("item")
                .bind(&song_tile, "song", glib::Object::NONE);
            gtk::ClosureExpression::new::<bool, _, _>(
                [
                    list_item.property_expression("selected"),
                    obj.property_expression("is-selection-mode"),
                ],
                closure!(
                    |_: Option<glib::Object>, selected: bool, is_selection_mode: bool| {
                        selected && is_selection_mode
                    }
                ),
            )
            .bind(&song_tile, "is-selected", glib::Object::NONE);
            list_item.set_child(Some(&song_tile));
        }));

        self.imp().grid.set_factory(Some(&factory));
    }
}

impl Default for HistoryView {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod test {
    use super::*;

    use gtk::gio;

    use std::sync::Once;

    use crate::{model::SongId, RESOURCES_FILE};

    static GRESOURCES_INIT: Once = Once::new();

    fn init_gresources() {
        GRESOURCES_INIT.call_once(|| {
            let res =
                gio::Resource::load(RESOURCES_FILE).expect("Tests could not load gresource file");
            gio::resources_register(&res);
        });
    }

    fn new_test_song(id: &str) -> Song {
        Song::builder(&SongId::from(id), id, id, id).build()
    }

    fn n_song_pages(view: &HistoryView) -> usize {
        view.imp().song_pages.borrow().len()
    }

    #[gtk::test]
    fn push_and_pop_song_page() {
        init_gresources();

        let player = Player::new();
        let song_list = SongList::default();

        let song = new_test_song("1");
        song_list.append(song.clone());

        let view = HistoryView::new();
        view.bind_player(&player);
        view.bind_song_list(&song_list);
        assert!(!view.is_on_song_page());
        assert_eq!(n_song_pages(&view), 0);

        view.push_song_page(&song);
        assert!(view.is_on_song_page());
        assert_eq!(n_song_pages(&view), 1);

        // Same song, n items should not change
        view.push_song_page(&song);
        assert_eq!(n_song_pages(&view), 1);

        view.pop_song_page();
        assert!(!view.is_on_song_page());
        assert_eq!(n_song_pages(&view), 0);

        // Popping with empty song pages should not do anything
        view.pop_song_page();
        assert!(!view.is_on_song_page());
        assert_eq!(n_song_pages(&view), 0);
    }

    #[gtk::test]
    fn push_and_pop_song_page_with_duplicate_non_adjacent() {
        init_gresources();

        let player = Player::new();
        let song_list = SongList::default();

        let song_1 = new_test_song("1");
        song_list.append(song_1.clone());
        let song_2 = new_test_song("2");
        song_list.append(song_2.clone());
        let song_3 = new_test_song("3");
        song_list.append(song_3.clone());

        let view = HistoryView::new();
        view.bind_player(&player);
        view.bind_song_list(&song_list);
        assert_eq!(n_song_pages(&view), 0);

        view.push_song_page(&song_1);
        assert_eq!(n_song_pages(&view), 1);

        view.push_song_page(&song_2);
        assert_eq!(n_song_pages(&view), 2);

        view.push_song_page(&song_3);
        assert_eq!(n_song_pages(&view), 3);

        // Even song_1 was already added, it is still
        // added as it is not adjacent to the other song_1
        view.push_song_page(&song_1);
        assert_eq!(n_song_pages(&view), 4);

        // Since song_1 is added twice, it should reduce
        // the number of pages by 2
        view.remove_song(&song_1);
        assert_eq!(n_song_pages(&view), 2);

        view.pop_song_page();
        assert_eq!(n_song_pages(&view), 1);

        view.remove_song(&song_2);
        assert_eq!(n_song_pages(&view), 0);
    }
}
