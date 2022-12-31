use adw::prelude::*;
use gettextrs::{gettext, ngettext};
use gtk::{
    gdk,
    glib::{self, clone, closure},
    subclass::prelude::*,
};
use once_cell::unsync::OnceCell;

use std::cell::{Cell, RefCell};

use super::{
    recognized_page::RecognizedPage, song_page::SongPage, song_tile::SongTile, AdaptiveMode,
};
use crate::{
    config::APP_ID,
    model::{FuzzyFilter, FuzzySorter, Song, SongList},
    player::Player,
    utils,
};

const SONG_PAGE_SONG_REMOVED_HANDLER_ID_KEY: &str = "mousai-song-page-song-removed-handler-id";
const SONG_PAGE_ADAPTIVE_MODE_BINDING_KEY: &str = "mousai-song-page-adapative-mode-binding";

const RECOGNIZED_PAGE_SONG_ACTIVATED_HANDLER_ID_KEY: &str =
    "mousai-recognized-page-song-activated-handler-id";
const RECOGNIZED_PAGE_ADAPTIVE_MODE_BINDING_KEY: &str =
    "mousai-recognized-page-adaptive-mode-binding";

const GRID_LIST_ITEM_BINDINGS_KEY: &str = "mousai-grid-list-item-bindings";
const GRID_LIST_ITEM_EXPRESSION_WATCHES_KEY: &str = "mousai-grid-list-item-expression-watches";

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

        pub(super) is_selection_mode_active: Cell<bool>,
        pub(super) adaptive_mode: Cell<AdaptiveMode>,

        pub(super) player: OnceCell<WeakRef<Player>>,
        pub(super) song_list: OnceCell<WeakRef<SongList>>,
        pub(super) filter_model: OnceCell<WeakRef<gtk::FilterListModel>>,
        pub(super) selection_model: OnceCell<WeakRef<gtk::MultiSelection>>,

        pub(super) removed_purgatory: RefCell<Vec<Song>>,
        pub(super) undo_remove_toast: RefCell<Option<adw::Toast>>,

        pub(super) extra_stack_items: RefCell<Vec<gtk::Widget>>,
        pub(super) pending_stack_remove: RefCell<Option<gtk::Widget>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for HistoryView {
        const NAME: &'static str = "MsaiHistoryView";
        type Type = super::HistoryView;
        type ParentType = gtk::Widget;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();

            klass.install_action("history-view.toggle-selection-mode", None, |obj, _, _| {
                // I don't know why exactly getting `is_selection_mode_active` first
                // before unselecting all, but it prevents flickering when cancelling
                // selection mode; probably, because we also set selection mode
                // on selection change callback.
                let is_selection_mode_active = obj.is_selection_mode_active();
                obj.unselect_all();
                obj.set_selection_mode_active(!is_selection_mode_active);
            });

            klass.install_action("history-view.select-all", None, |obj, _, _| {
                obj.select_all();
            });

            klass.install_action("history-view.select-none", None, |obj, _, _| {
                obj.unselect_all();
            });

            klass.install_action("history-view.copy-selected-song", None, |obj, _, _| {
                let selected_songs = obj.snapshot_selected_songs();

                if selected_songs.len() != 1 {
                    tracing::error!(
                        "Copying should only be allowed if there is exactly one selected"
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
                    // Whether selection mode is active
                    glib::ParamSpecBoolean::builder("selection-mode-active")
                        .read_only()
                        .build(),
                    // Current adapative mode
                    glib::ParamSpecEnum::builder("adaptive-mode", AdaptiveMode::default())
                        .explicit_notify()
                        .build(),
                ]
            });

            PROPERTIES.as_ref()
        }

        fn set_property(&self, _id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
            let obj = self.obj();

            match pspec.name() {
                "adaptive-mode" => {
                    let adaptive_mode = value.get().unwrap();
                    obj.set_adaptive_mode(adaptive_mode);
                }
                _ => unimplemented!(),
            }
        }

        fn property(&self, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
            let obj = self.obj();

            match pspec.name() {
                "selection-mode-active" => obj.is_selection_mode_active().to_value(),
                "adaptive-mode" => obj.adaptive_mode().to_value(),
                _ => unimplemented!(),
            }
        }

        fn constructed(&self) {
            self.parent_constructed();

            let obj = self.obj();

            self.stack
                .connect_transition_running_notify(clone!(@weak obj => move |stack| {
                    let imp = obj.imp();
                    if !stack.is_transition_running() {
                        if let Some(song_page) = imp.pending_stack_remove.take() {
                            stack.remove(&song_page);
                        }
                    }
                }));

            self.empty_page.set_icon_name(Some(APP_ID));
            obj.setup_grid();

            obj.update_selection_actions();
            obj.update_selection_mode_ui();

            obj.update_history_stack_visible_child();
        }

        fn dispose(&self) {
            while let Some(child) = self.obj().first_child() {
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
        glib::Object::builder().build()
    }

    pub fn connect_selection_mode_active_notify<F>(&self, f: F) -> glib::SignalHandlerId
    where
        F: Fn(&Self) + 'static,
    {
        self.connect_notify_local(Some("selection-mode-active"), move |obj, _| f(obj))
    }

    pub fn is_selection_mode_active(&self) -> bool {
        self.imp().is_selection_mode_active.get()
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
        self.set_selection_mode_active(false);
    }

    pub fn search_bar(&self) -> gtk::SearchBar {
        self.imp().search_bar.get()
    }

    pub fn is_on_main_stack_main_page(&self) -> bool {
        let imp = self.imp();
        let no_extra_stack_items = imp.extra_stack_items.borrow().is_empty();

        if no_extra_stack_items {
            debug_assert!(imp
                .stack
                .visible_child()
                .map_or(false, |child| child.is::<gtk::Box>()));
        }

        no_extra_stack_items
    }

    pub fn push_recognized_page(&self, songs: &[Song]) {
        let imp = self.imp();

        if imp
            .extra_stack_items
            .borrow()
            .iter()
            .any(|widget| widget.is::<RecognizedPage>())
        {
            tracing::warn!("There is already a `RecognizedPage` on the stack");
            return;
        }

        let recognized_page = RecognizedPage::new();
        recognized_page.bind_player(&self.player());
        recognized_page.bind_songs(songs);

        let song_activated_handler_id =
            recognized_page.connect_song_activated(clone!(@weak self as obj => move |_, song| {
                obj.push_song_page(song);
            }));
        let adaptive_mode_binding = self
            .bind_property("adaptive-mode", &recognized_page, "adaptive-mode")
            .sync_create()
            .build();

        unsafe {
            recognized_page.set_data(
                RECOGNIZED_PAGE_SONG_ACTIVATED_HANDLER_ID_KEY,
                song_activated_handler_id,
            );
            recognized_page.set_data(
                RECOGNIZED_PAGE_ADAPTIVE_MODE_BINDING_KEY,
                adaptive_mode_binding,
            );
        }

        imp.stack.add_child(&recognized_page);
        imp.stack.set_visible_child(&recognized_page);

        imp.extra_stack_items
            .borrow_mut()
            .push(recognized_page.upcast());
    }

    pub fn push_song_page(&self, song: &Song) {
        let imp = self.imp();

        // Return if the last widget is a `SongPage` and its song is the same as the given song
        if let Some(widget) = imp.extra_stack_items.borrow().last() {
            if let Some(song_page) = widget.downcast_ref::<SongPage>() {
                if Some(song.id()) == song_page.song().map(|song| song.id()) {
                    return;
                }
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
            .sync_create()
            .build();

        imp.stack.add_child(&song_page);
        imp.stack.set_visible_child(&song_page);

        unsafe {
            song_page.set_data(
                SONG_PAGE_SONG_REMOVED_HANDLER_ID_KEY,
                song_removed_handler_id,
            );
            song_page.set_data(SONG_PAGE_ADAPTIVE_MODE_BINDING_KEY, adaptive_mode_binding);
        }

        imp.extra_stack_items.borrow_mut().push(song_page.upcast());

        // User is already aware of the newly recognized song, so unset it.
        song.set_newly_heard(false);
    }

    pub fn pop_stack_item(&self) {
        let imp = self.imp();

        let Some(item) = imp.extra_stack_items.borrow_mut().pop() else {
            self.update_history_stack_visible_child();
            if imp.stack.visible_child().as_ref() != Some(&imp.history_child.get().upcast()) {
                tracing::error!(
                    "Popped all extra stack items, but the history child is still not the visible child"
                );
            }
            return;
        };

        self.update_stack_visible_child();

        if let Some(song_page) = item.downcast_ref::<SongPage>() {
            unbind_song_page(song_page);
        } else if let Some(recognized_page) = item.downcast_ref::<RecognizedPage>() {
            unsafe {
                let song_activated_handler_id = recognized_page
                    .steal_data::<glib::SignalHandlerId>(
                        RECOGNIZED_PAGE_SONG_ACTIVATED_HANDLER_ID_KEY,
                    )
                    .unwrap();
                recognized_page.disconnect(song_activated_handler_id);

                let adaptive_mode_binding = recognized_page
                    .steal_data::<glib::Binding>(RECOGNIZED_PAGE_ADAPTIVE_MODE_BINDING_KEY)
                    .unwrap();
                adaptive_mode_binding.unbind();
            }
            recognized_page.unbind_player();
        } else {
            tracing::error!("Unknown extra stack item type");
        }

        self.replace_pending_stack_remove(item);
    }

    /// Must only be called once
    pub fn bind_player(&self, player: &Player) {
        self.imp().player.set(player.downgrade()).unwrap();
    }

    /// Must only be called once
    pub fn bind_song_list(&self, song_list: &SongList) {
        let imp = self.imp();

        song_list.connect_items_changed(clone!(@weak self as obj => move |_, _, _, _| {
            obj.update_history_stack_visible_child();
        }));

        let filter = FuzzyFilter::new();
        let sorter = FuzzySorter::new();

        let filter_model = gtk::FilterListModel::new(Some(song_list), Some(&filter));
        filter_model.connect_items_changed(clone!(@weak self as obj => move |_, _, _, _| {
            obj.update_history_stack_visible_child();
        }));

        imp.search_entry.connect_search_changed(
            clone!(@weak self as obj, @weak filter, @weak sorter => move |search_entry| {
                let text = search_entry.text();
                filter.set_search(&text);
                sorter.set_search(&text);
                obj.update_history_stack_visible_child();
            }),
        );

        let sort_model = gtk::SortListModel::new(Some(&filter_model), Some(&sorter));

        // FIXME save selection even when the song are filtered from FilterListModel
        let selection_model = gtk::MultiSelection::new(Some(&sort_model));
        selection_model.connect_selection_changed(clone!(@weak self as obj => move |model, _, _| {
            if obj.is_selection_mode_active() {
                if model.selection().size() == 0 {
                    obj.set_selection_mode_active(false);
                }

                obj.update_selection_actions();
            }
        }));
        selection_model.connect_items_changed(clone!(@weak self as obj => move |model, _, _, _| {
            if obj.is_selection_mode_active() {
                if model.selection().size() == 0 {
                    obj.set_selection_mode_active(false);
                }

                obj.update_selection_actions();
            }
        }));

        let grid = imp.grid.get();
        grid.set_model(Some(&selection_model));
        grid.connect_activate(
            clone!(@weak self as obj, @weak selection_model => move |_, index| {
                match selection_model.item(index) {
                    Some(ref item) => {
                        let song = item.downcast_ref::<Song>().unwrap();
                        obj.push_song_page(song);
                    }
                    None => tracing::error!("Activated `{index}`, but found no song.")
                }
            }),
        );

        imp.song_list.set(song_list.downgrade()).unwrap();
        imp.filter_model.set(filter_model.downgrade()).unwrap();
        imp.selection_model
            .set(selection_model.downgrade())
            .unwrap();

        self.update_history_stack_visible_child();
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
        let (to_drain_items, to_retain_items) = imp
            .extra_stack_items
            .take()
            .into_iter()
            // FIXME use Vec::drain_filter
            .partition(|item| {
                if let Some(song_page) = item.downcast_ref::<SongPage>() {
                    // drain if it matches
                    song_page.song().map(|song_page_song| song_page_song.id()) == Some(song.id())
                } else {
                    // retain
                    false
                }
            });
        imp.extra_stack_items.replace(to_retain_items);

        self.update_stack_visible_child();

        for item in to_drain_items {
            unbind_song_page(item.downcast_ref().unwrap());
            self.replace_pending_stack_remove(item);
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

    fn set_selection_mode_active(&self, is_selection_mode_active: bool) {
        if is_selection_mode_active == self.is_selection_mode_active() {
            return;
        }

        self.imp()
            .is_selection_mode_active
            .set(is_selection_mode_active);
        self.update_selection_mode_ui();
        self.update_selection_actions();

        self.notify("selection-mode-active");
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

    fn replace_pending_stack_remove(&self, widget: gtk::Widget) {
        let imp = self.imp();

        if let Some(item) = imp.pending_stack_remove.replace(Some(widget)) {
            imp.stack.remove(&item);
        }
    }

    fn update_selection_mode_ui(&self) {
        let imp = self.imp();
        let is_selection_mode_active = self.is_selection_mode_active();

        if is_selection_mode_active {
            imp.header_bar_stack
                .set_visible_child(&imp.selection_mode_header_bar.get());
            imp.grid.grab_focus();
        } else {
            imp.header_bar_stack
                .set_visible_child(&imp.main_header_bar.get());
        }

        imp.selection_mode_bar
            .set_revealed(is_selection_mode_active);
        imp.grid.set_enable_rubberband(is_selection_mode_active);
        imp.grid
            .set_single_click_activate(!is_selection_mode_active);
    }

    fn update_stack_visible_child(&self) {
        let imp = self.imp();

        imp.stack
            .set_visible_child(&imp.extra_stack_items.borrow().last().map_or_else(
                || imp.history_child.get().upcast::<gtk::Widget>(),
                |item| item.clone(),
            ));
    }

    fn update_history_stack_visible_child(&self) {
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
            let list_item = list_item.downcast_ref::<gtk::ListItem>().unwrap();

            let song_tile = SongTile::new();
            song_tile.set_show_select_button_on_hover(true);
            song_tile.bind_player(&obj.player());

            let selection_mode_active_binding = obj
                .bind_property("selection-mode-active", &song_tile, "selection-mode-active")
                .sync_create()
                .build();
            let adaptive_mode_binding = obj
                .bind_property("adaptive-mode", &song_tile, "adaptive-mode")
                .sync_create()
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
                obj.set_selection_mode_active(true);
            }));

            let song_watch =
                list_item
                    .property_expression("item")
                    .bind(&song_tile, "song", glib::Object::NONE);
            let selected_watch = gtk::ClosureExpression::new::<bool>(
                [
                    list_item.property_expression("selected"),
                    obj.property_expression("selection-mode-active"),
                ],
                closure!(|_: Option<glib::Object>,
                        is_selected: bool,
                        is_selection_mode_active: bool| {
                    is_selected && is_selection_mode_active
                }),
            )
            .bind(&song_tile, "selected", glib::Object::NONE);

            unsafe {
                list_item.set_data(
                    GRID_LIST_ITEM_BINDINGS_KEY,
                    vec![selection_mode_active_binding, adaptive_mode_binding],
                );
                list_item.set_data(
                    GRID_LIST_ITEM_EXPRESSION_WATCHES_KEY,
                    vec![song_watch, selected_watch],
                );
            }

            list_item.set_child(Some(&song_tile));
        }));
        factory.connect_teardown(|_, list_item| {
            let list_item = list_item.downcast_ref::<gtk::ListItem>().unwrap();

            unsafe {
                let bindings = list_item
                    .steal_data::<Vec<glib::Binding>>(GRID_LIST_ITEM_BINDINGS_KEY)
                    .unwrap();
                for binding in bindings {
                    binding.unbind();
                }

                let watches = list_item
                    .steal_data::<Vec<gtk::ExpressionWatch>>(GRID_LIST_ITEM_EXPRESSION_WATCHES_KEY)
                    .unwrap();
                for watch in watches {
                    watch.unwatch();
                }
            }
        });

        self.imp().grid.set_factory(Some(&factory));
    }
}

impl Default for HistoryView {
    fn default() -> Self {
        Self::new()
    }
}

fn unbind_song_page(song_page: &SongPage) {
    unsafe {
        let handler_id = song_page
            .steal_data::<glib::SignalHandlerId>(SONG_PAGE_SONG_REMOVED_HANDLER_ID_KEY)
            .unwrap();
        song_page.disconnect(handler_id);

        let binding = song_page
            .steal_data::<glib::Binding>(SONG_PAGE_ADAPTIVE_MODE_BINDING_KEY)
            .unwrap();
        binding.unbind();
    };

    song_page.unbind_player();
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

    fn n_extra_stack_items(view: &HistoryView) -> usize {
        view.imp().extra_stack_items.borrow().len()
    }

    #[gtk::test]
    fn push_and_pop_stack_item() {
        init_gresources();
        gst::init().unwrap(); // For Player

        let player = Player::new();
        let song_list = SongList::default();

        let song = new_test_song("1");
        song_list.append(song.clone());

        let view = HistoryView::new();
        view.bind_player(&player);
        view.bind_song_list(&song_list);
        assert!(view.is_on_main_stack_main_page());
        assert_eq!(n_extra_stack_items(&view), 0);

        view.push_song_page(&song);
        assert!(!view.is_on_main_stack_main_page());
        assert_eq!(n_extra_stack_items(&view), 1);

        // Same song, n items should not change
        view.push_song_page(&song);
        assert!(!view.is_on_main_stack_main_page());
        assert_eq!(n_extra_stack_items(&view), 1);

        view.push_recognized_page(&[]);
        assert!(!view.is_on_main_stack_main_page());
        assert_eq!(n_extra_stack_items(&view), 2);

        view.pop_stack_item();
        assert!(!view.is_on_main_stack_main_page());
        assert_eq!(n_extra_stack_items(&view), 1);

        view.pop_stack_item();
        assert!(view.is_on_main_stack_main_page());
        assert_eq!(n_extra_stack_items(&view), 0);

        // Popping with empty song pages should not do anything
        view.pop_stack_item();
        assert!(view.is_on_main_stack_main_page());
        assert_eq!(n_extra_stack_items(&view), 0);
    }

    #[gtk::test]
    fn push_and_pop_song_page_with_duplicate_non_adjacent() {
        init_gresources();
        gst::init().unwrap(); // For Player

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
        assert_eq!(n_extra_stack_items(&view), 0);

        view.push_song_page(&song_1);
        assert_eq!(n_extra_stack_items(&view), 1);

        view.push_song_page(&song_2);
        assert_eq!(n_extra_stack_items(&view), 2);

        view.push_song_page(&song_3);
        assert_eq!(n_extra_stack_items(&view), 3);

        // Even song_1 was already added, it is still
        // added as it is not adjacent to the other song_1
        view.push_song_page(&song_1);
        assert_eq!(n_extra_stack_items(&view), 4);

        // Since song_1 is added twice, it should reduce
        // the number of pages by 2
        view.remove_song(&song_1);
        assert_eq!(n_extra_stack_items(&view), 2);

        view.pop_stack_item();
        assert_eq!(n_extra_stack_items(&view), 1);

        view.remove_song(&song_2);
        assert_eq!(n_extra_stack_items(&view), 0);
    }

    #[gtk::test]
    fn push_and_pop_song_page_with_recognized_page_in_between() {
        init_gresources();
        gst::init().unwrap(); // For Player

        let player = Player::new();
        let song_list = SongList::default();

        let song_1 = new_test_song("1");
        song_list.append(song_1.clone());

        let view = HistoryView::new();
        view.bind_player(&player);
        view.bind_song_list(&song_list);
        assert_eq!(n_extra_stack_items(&view), 0);

        view.push_song_page(&song_1);
        assert_eq!(n_extra_stack_items(&view), 1);

        view.push_recognized_page(&[]);
        assert_eq!(n_extra_stack_items(&view), 2);

        // Even song_1 was already added, it is still
        // added as there is a RecognizedPage in between
        view.push_song_page(&song_1);
        assert_eq!(n_extra_stack_items(&view), 3);

        // Since song_1 is added twice, it should reduce
        // the number of pages by 2
        view.remove_song(&song_1);
        assert_eq!(n_extra_stack_items(&view), 1);

        view.pop_stack_item();
        assert_eq!(n_extra_stack_items(&view), 0);
    }
}
