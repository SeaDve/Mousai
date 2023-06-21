use adw::prelude::*;
use anyhow::{Context, Result};
use gettextrs::{gettext, ngettext};
use gtk::{
    glib::{self, clone, closure},
    subclass::prelude::*,
};
use once_cell::unsync::OnceCell;

use std::cell::{Cell, RefCell};

use super::{
    recognized_page::RecognizedPage, recognizer_status::RecognizerStatus, song_page::SongPage,
    song_tile::SongTile, AdaptiveMode,
};
use crate::{
    config::APP_ID,
    i18n::ngettext_f,
    model::{Song, SongFilter, SongList, SongSorter, Uid},
    player::Player,
    recognizer::Recognizer,
    utils,
};

// FIXME
// * Missing global navigation shortcuts
// * Missing title on main navigation page

const SONG_PAGE_SONG_REMOVE_REQUEST_HANDLER_ID_KEY: &str =
    "mousai-song-page-song-remove-request-handler-id";
const SONG_PAGE_ADAPTIVE_MODE_BINDING_KEY: &str = "mousai-song-page-adaptive-mode-binding";

const RECOGNIZED_PAGE_SONG_ACTIVATED_HANDLER_ID_KEY: &str =
    "mousai-recognized-page-song-activated-handler-id";
const RECOGNIZED_PAGE_ADAPTIVE_MODE_BINDING_KEY: &str =
    "mousai-recognized-page-adaptive-mode-binding";

const GRID_LIST_ITEM_BINDINGS_KEY: &str = "mousai-grid-list-item-bindings";
const GRID_LIST_ITEM_EXPRESSION_WATCHES_KEY: &str = "mousai-grid-list-item-expression-watches";

mod imp {
    use super::*;
    use glib::WeakRef;

    #[derive(Default, glib::Properties, gtk::CompositeTemplate)]
    #[properties(wrapper_type = super::HistoryView)]
    #[template(resource = "/io/github/seadve/Mousai/ui/history-view.ui")]
    pub struct HistoryView {
        /// Whether selection mode is active
        #[property(get)]
        pub(super) is_selection_mode_active: Cell<bool>,
        /// Current adaptive mode
        #[property(get, set = Self::set_adaptive_mode, explicit_notify, builder(AdaptiveMode::default()))]
        pub(super) adaptive_mode: Cell<AdaptiveMode>,

        #[template_child]
        pub(super) navigation_view: TemplateChild<adw::NavigationView>,
        #[template_child]
        pub(super) navigation_main_page: TemplateChild<adw::NavigationPage>,
        #[template_child]
        pub(super) header_bar_stack: TemplateChild<gtk::Stack>,
        #[template_child]
        pub(super) main_header_bar: TemplateChild<adw::HeaderBar>,
        #[template_child]
        pub(super) recognizer_status: TemplateChild<RecognizerStatus>,
        #[template_child]
        pub(super) selection_mode_header_bar: TemplateChild<adw::HeaderBar>,
        #[template_child]
        pub(super) selection_mode_menu_button: TemplateChild<gtk::MenuButton>,
        #[template_child]
        pub(super) selection_mode_bar: TemplateChild<gtk::ActionBar>,
        #[template_child]
        pub(super) copy_selected_songs_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub(super) remove_selected_songs_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub(super) search_bar: TemplateChild<gtk::SearchBar>,
        #[template_child]
        pub(super) search_entry: TemplateChild<gtk::SearchEntry>,
        #[template_child]
        pub(super) content_stack: TemplateChild<gtk::Stack>,
        #[template_child]
        pub(super) content_main_page: TemplateChild<gtk::ScrolledWindow>,
        #[template_child]
        pub(super) grid: TemplateChild<gtk::GridView>,
        #[template_child]
        pub(super) content_empty_page: TemplateChild<adw::StatusPage>,
        #[template_child]
        pub(super) content_empty_search_result_page: TemplateChild<adw::StatusPage>,

        pub(super) player: OnceCell<WeakRef<Player>>,
        pub(super) song_list: OnceCell<WeakRef<SongList>>,
        pub(super) filter_model: OnceCell<WeakRef<gtk::FilterListModel>>,
        pub(super) selection_model: OnceCell<WeakRef<gtk::MultiSelection>>,

        pub(super) songs_purgatory: RefCell<Vec<Song>>,
        pub(super) undo_remove_song_toast: RefCell<Option<adw::Toast>>,

        pub(super) navigation_forward_stack: RefCell<Vec<adw::NavigationPage>>,
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

                debug_assert!(
                    !selected_songs.is_empty(),
                    "copying must only be allowed if there is atleast one selected"
                );

                let text = selected_songs
                    .iter()
                    .map(|song| song.copy_term())
                    .collect::<Vec<_>>()
                    .join("\n");
                obj.display().clipboard().set_text(&text);

                utils::app_instance()
                    .window()
                    .add_message_toast(&gettext("Copied to clipboard"));
            });

            klass.install_action("history-view.remove-selected-songs", None, |obj, _, _| {
                let selected_songs = obj.snapshot_selected_songs();
                let song_ids = selected_songs
                    .iter()
                    .map(|song| song.id_ref())
                    .collect::<Vec<_>>();

                if let Err(err) = obj.remove_songs(&song_ids) {
                    tracing::error!("Failed to remove songs: {:?}", err);
                    utils::app_instance()
                        .window()
                        .add_message_toast(&gettext("Failed to remove selected songs"));
                    return;
                }

                obj.show_undo_remove_song_toast();
            });
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for HistoryView {
        crate::derived_properties!();

        fn constructed(&self) {
            self.parent_constructed();

            let obj = self.obj();

            self.navigation_view
                .connect_pushed(clone!(@weak obj => move |view| {
                    let imp = obj.imp();

                    let visible_page = view
                        .visible_page()
                        .expect("visible page must exist on pushed");
                    let forward_page = imp.navigation_forward_stack.borrow().last().cloned();

                    if let Some(forward_page) = forward_page {
                        if visible_page == forward_page {
                            // The page is already on the navigation stack, so remove
                            // it from the forward stack
                            let removed = imp.navigation_forward_stack.borrow_mut().pop();
                            debug_assert_eq!(removed.as_ref(), Some(&forward_page));
                        } else {
                            for page in imp.navigation_forward_stack.take() {
                                unbind_page(&page);
                            }
                        }
                    }
                }));
            self.navigation_view
                .connect_popped(clone!(@weak obj => move |_, page| {
                    obj.imp()
                        .navigation_forward_stack
                        .borrow_mut()
                        .push(page.clone());
                }));
            self.navigation_view.connect_get_next_page(
                clone!(@weak obj => @default-panic, move |_| {
                    let next_page = obj.imp()
                        .navigation_forward_stack
                        .borrow()
                        .last()
                        .cloned();

                    next_page
                }),
            );

            self.content_empty_page.set_icon_name(Some(APP_ID));
            obj.setup_grid();

            obj.update_selection_actions();
            obj.update_selection_mode_ui();

            obj.update_content_stack_visible_child();
        }

        fn dispose(&self) {
            self.dispose_template();
        }
    }

    impl WidgetImpl for HistoryView {}

    impl HistoryView {
        fn set_adaptive_mode(&self, adaptive_mode: AdaptiveMode) {
            let obj = self.obj();

            if adaptive_mode == obj.adaptive_mode() {
                return;
            }

            self.adaptive_mode.set(adaptive_mode);
            obj.notify_adaptive_mode();
        }
    }
}

glib::wrapper! {
    pub struct HistoryView(ObjectSubclass<imp::HistoryView>)
        @extends gtk::Widget;
}

impl HistoryView {
    pub fn new() -> Self {
        glib::Object::new()
    }

    pub fn stop_selection_mode(&self) {
        self.set_selection_mode_active(false);
    }

    pub fn search_bar(&self) -> gtk::SearchBar {
        self.imp().search_bar.get()
    }

    pub fn is_on_navigation_main_page(&self) -> bool {
        let imp = self.imp();
        imp.navigation_view.visible_page().as_ref() == Some(imp.navigation_main_page.upcast_ref())
    }

    /// Pushes a `RecognizedPage` for the given songs to the navigation stack.
    pub fn push_recognized_page(&self, songs: &[Song]) {
        let imp = self.imp();

        debug_assert!(
            !imp.navigation_view
                .navigation_stack()
                .iter::<adw::NavigationPage>()
                .map(|page| page.unwrap())
                .any(|page| page.is::<RecognizedPage>()),
            "there must not be already a `RecognizedPage` on the navigation stack"
        );

        let recognized_page = RecognizedPage::new();
        recognized_page.bind_player(&self.player());
        recognized_page.bind_songs(songs);

        unsafe {
            recognized_page.set_data(
                RECOGNIZED_PAGE_SONG_ACTIVATED_HANDLER_ID_KEY,
                recognized_page.connect_song_activated(
                    clone!(@weak self as obj => move |_, song| {
                        obj.push_song_page(song);
                    }),
                ),
            );
            recognized_page.set_data(
                RECOGNIZED_PAGE_ADAPTIVE_MODE_BINDING_KEY,
                self.bind_property("adaptive-mode", &recognized_page, "adaptive-mode")
                    .sync_create()
                    .build(),
            );
        }

        imp.navigation_view.push(&recognized_page);
    }

    /// Pushes a `SongPage` for the given song to the navigation stack.
    pub fn push_song_page(&self, song: &Song) {
        let imp = self.imp();

        // Return if the last widget is a `SongPage` and its song is the same as the given song
        if let Some(visible_page) = imp.navigation_view.visible_page() {
            if let Some(song_page) = visible_page.downcast_ref::<SongPage>() {
                if Some(song.id_ref()) == song_page.song().map(|song| song.id()).as_ref() {
                    return;
                }
            }
        }

        let song_page = SongPage::new();
        song_page.bind_player(&self.player());
        song_page.bind_song_list(&self.song_list());
        song_page.set_song(song);

        unsafe {
            song_page.set_data(
                SONG_PAGE_SONG_REMOVE_REQUEST_HANDLER_ID_KEY,
                song_page.connect_song_remove_request(clone!(@weak self as obj => move |_, song| {
                    if let Err(err) = obj.remove_songs(&[song.id_ref()]) {
                        tracing::error!("Failed to remove song: {:?}", err);
                        utils::app_instance()
                            .window()
                            .add_message_toast(&gettext("Failed to remove song"));
                        return;
                    }

                    obj.show_undo_remove_song_toast();
                })),
            );
            song_page.set_data(
                SONG_PAGE_ADAPTIVE_MODE_BINDING_KEY,
                self.bind_property("adaptive-mode", &song_page, "adaptive-mode")
                    .sync_create()
                    .build(),
            );
        }

        imp.navigation_view.push(&song_page);

        // User is already aware of the newly recognized song, so unset it.
        song.set_is_newly_heard(false);
    }

    /// Returns true if a page has been popped
    pub fn pop_page(&self) -> bool {
        self.imp().navigation_view.pop()
    }

    /// Must only be called once
    pub fn bind_player(&self, player: &Player) {
        self.imp().player.set(player.downgrade()).unwrap();
    }

    /// Must only be called once
    pub fn bind_song_list(&self, song_list: &SongList) {
        let imp = self.imp();

        song_list.connect_items_changed(clone!(@weak self as obj => move |_, _, _, _| {
            obj.update_content_stack_visible_child();
        }));

        let filter = SongFilter::new();
        let sorter = SongSorter::new();

        let filter_model = gtk::FilterListModel::new(Some(song_list.clone()), Some(filter.clone()));
        filter_model.connect_items_changed(clone!(@weak self as obj => move |_, _, _, _| {
            obj.update_content_stack_visible_child();
        }));

        imp.search_entry.connect_search_changed(
            clone!(@weak self as obj, @weak filter, @weak sorter => move |search_entry| {
                let text = search_entry.text();
                filter.set_search(text.trim());
                sorter.set_search(text.trim());
                obj.update_content_stack_visible_child();
            }),
        );

        let sort_model = gtk::SortListModel::new(Some(filter_model.clone()), Some(sorter));

        // FIXME save selection even when the song are filtered from FilterListModel
        let selection_model = gtk::MultiSelection::new(Some(sort_model));
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
                    None => unreachable!("selection model must have item at index `{}`", index)
                }
            }),
        );

        imp.song_list.set(song_list.downgrade()).unwrap();
        imp.filter_model.set(filter_model.downgrade()).unwrap();
        imp.selection_model
            .set(selection_model.downgrade())
            .unwrap();

        self.update_content_stack_visible_child();
    }

    /// Must only be called once
    pub fn bind_recognizer(&self, recognizer: &Recognizer) {
        let imp = self.imp();

        imp.recognizer_status.bind_recognizer(recognizer);

        imp.recognizer_status.connect_show_results_requested(
            clone!(@weak self as obj, @weak recognizer => move |_| {
                if let Err(err) = obj.show_recognizer_results(&recognizer) {
                    tracing::error!("Failed to show recognizer results: {:?}", err);
                    utils::app_instance()
                        .window()
                        .add_message_toast(&gettext("Failed to show recognizer results"));
                }
            }),
        );
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
            .expect("player must be bound")
            .upgrade()
            .expect("player must not be dropped")
    }

    fn song_list(&self) -> SongList {
        self.imp()
            .song_list
            .get()
            .expect("song list must be bound")
            .upgrade()
            .expect("song list must not be dropped")
    }

    /// Adds songs with ids given to purgatory, and add `SongPage`s that contain them to the purgatory.
    fn remove_songs(&self, song_ids: &[&Uid]) -> Result<()> {
        let imp = self.imp();

        let mut removed_songs = self
            .song_list()
            .remove_many(song_ids)
            .context("Failed to remove songs from history")?;
        debug_assert_eq!(
            removed_songs.len(),
            song_ids.len(),
            "all corresponding songs of the ids must be removed"
        );
        imp.songs_purgatory.borrow_mut().append(&mut removed_songs);

        Ok(())
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
        let selection_model = self
            .imp()
            .selection_model
            .get()
            .and_then(|model| model.upgrade())
            .expect("selection model should exist");
        selection_model.select_all();
    }

    fn unselect_all(&self) {
        let selection_model = self
            .imp()
            .selection_model
            .get()
            .and_then(|model| model.upgrade())
            .expect("selection model should exist");
        selection_model.unselect_all();
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

        self.notify_is_selection_mode_active();
    }

    fn show_undo_remove_song_toast(&self) {
        let imp = self.imp();

        if imp.undo_remove_song_toast.borrow().is_none() {
            let toast = adw::Toast::builder()
                .priority(adw::ToastPriority::High)
                .button_label(gettext("_Undo"))
                .build();

            toast.connect_button_clicked(clone!(@weak self as obj => move |_| {
                if let Err(err) = obj
                    .song_list()
                    .insert_many(obj.imp().songs_purgatory.take())
                {
                    tracing::error!("Failed to undo remove song: {:?}", err);
                    utils::app_instance()
                        .window()
                        .add_message_toast(&gettext("Failed to undo"));
                }
            }));

            toast.connect_dismissed(clone!(@weak self as obj => move |_| {
                let imp = obj.imp();
                imp.songs_purgatory.borrow_mut().clear();
                imp.undo_remove_song_toast.take();
            }));

            utils::app_instance().window().add_toast(toast.clone());

            imp.undo_remove_song_toast.replace(Some(toast));
        }

        // Add this point we should already have a toast setup
        if let Some(ref toast) = *imp.undo_remove_song_toast.borrow() {
            let n_removed = imp.songs_purgatory.borrow().len();
            toast.set_title(&ngettext_f(
                // Translators: Do NOT translate the contents between '{' and '}', this is a variable name.
                "Removed {n_removed} song",
                "Removed {n_removed} songs",
                n_removed as u32,
                &[("n_removed", &n_removed.to_string())],
            ));

            // Reset toast timeout
            utils::app_instance().window().add_toast(toast.clone());
        }
    }

    fn show_recognizer_results(&self, recognizer: &Recognizer) -> Result<()> {
        let song_list = self.song_list();

        let songs = recognizer
            .take_recognized_saved_recordings()
            .context("Failed to take recognized saved recordings")?
            .iter()
            .filter_map(
                |recording| match recording.recognize_result().map(|r| r.0) {
                    Some(Ok(ref song)) => Some(song.clone()),
                    Some(Err(ref err)) => {
                        // TODO handle errors
                        debug_assert!(
                            err.is_permanent(),
                            "only results with permanent errors must be taken"
                        );
                        None
                    }
                    None => unreachable!("recognized saved recordings should have some result"),
                },
            )
            .collect::<Vec<_>>();

        if songs.is_empty() {
            tracing::debug!("No saved recordings taken when requested");
            return Ok(());
        }

        for song in &songs {
            // If the song is not found in the history, set it as newly heard
            // (That's why an always true value is used after `or`). If it is in the
            // history and it was newly heard, pass that state to the new value.
            if song_list
                .get(song.id_ref())
                .map_or(true, |prev| prev.is_newly_heard())
            {
                song.set_is_newly_heard(true);
            }
        }

        song_list
            .insert_many(songs.clone())
            .context("Failed to insert songs to history")?;

        self.push_recognized_page(&songs);
        self.scroll_to_top();

        Ok(())
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

    fn update_content_stack_visible_child(&self) {
        let imp = self.imp();

        let search_text = imp.search_entry.text();

        if imp
            .filter_model
            .get()
            .and_then(|filter_model| filter_model.upgrade())
            .map_or(true, |filter_model| filter_model.n_items() == 0)
            && !search_text.is_empty()
        {
            imp.content_stack
                .set_visible_child(&imp.content_empty_search_result_page.get());
        } else if imp
            .song_list
            .get()
            .and_then(|song_list| song_list.upgrade())
            .map_or(true, |song_list| song_list.n_items() == 0)
            && search_text.is_empty()
        {
            imp.content_stack
                .set_visible_child(&imp.content_empty_page.get());
        } else {
            imp.content_stack
                .set_visible_child(&imp.content_main_page.get());
        }
    }

    fn update_selection_actions(&self) {
        let imp = self.imp();
        let selection_size = imp
            .selection_model
            .get()
            .and_then(|model| model.upgrade())
            .map_or(0, |model| model.selection().size());

        self.action_set_enabled("history-view.copy-selected-song", selection_size != 0);
        self.action_set_enabled("history-view.remove-selected-songs", selection_size != 0);

        imp.selection_mode_menu_button
            .set_label(&match selection_size {
                0 => gettext("Select items"),
                1.. => {
                    ngettext_f(
                        // Translators: Do NOT translate the contents between '{' and '}', this is a variable name.
                        "Selected {selection_size} song",
                        "Selected {selection_size} songs",
                        selection_size as u32,
                        &[("selection_size", &selection_size.to_string())],
                    )
                }
            });

        imp.copy_selected_songs_button
            .set_tooltip_text(Some(&ngettext(
                "Copy Song",
                "Copy Songs",
                selection_size as u32,
            )));

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
            song_tile.set_shows_select_button_on_hover(true);
            song_tile.bind_player(&obj.player());

            let selection_mode_active_binding = obj
                .bind_property("is-selection-mode-active", &song_tile, "is-selection-mode-active")
                .sync_create()
                .build();
            let adaptive_mode_binding = obj
                .bind_property("adaptive-mode", &song_tile, "adaptive-mode")
                .sync_create()
                .build();

            song_tile.connect_is_active_notify(clone!(@weak obj, @weak list_item => move |tile| {
                let selection_model = obj
                    .imp()
                    .selection_model
                    .get()
                    .and_then(|model| model.upgrade())
                    .expect("selection model should exist");
                if tile.is_active() {
                    selection_model.select_item(list_item.position(), false);
                } else {
                    selection_model.unselect_item(list_item.position());
                }
            }));
            song_tile.connect_selection_mode_requested(clone!(@weak obj => move |_| {
                obj.set_selection_mode_active(true);
            }));

            let song_watch =
                list_item
                    .property_expression("item")
                    .bind(&song_tile, "song", glib::Object::NONE);
            let selected_watch = gtk::ClosureExpression::new::<bool>(
                [
                    list_item.property_expression("selected"),
                    obj.property_expression("is-selection-mode-active"),
                ],
                closure!(|_: Option<glib::Object>,
                        is_selected: bool,
                        is_selection_mode_active: bool| {
                    is_selected && is_selection_mode_active
                }),
            )
            .bind(&song_tile, "is-selected", glib::Object::NONE);

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

fn unbind_page(page: &adw::NavigationPage) {
    if let Some(song_page) = page.downcast_ref::<SongPage>() {
        unsafe {
            let handler_id = song_page
                .steal_data::<glib::SignalHandlerId>(SONG_PAGE_SONG_REMOVE_REQUEST_HANDLER_ID_KEY)
                .unwrap();
            song_page.disconnect(handler_id);

            let binding = song_page
                .steal_data::<glib::Binding>(SONG_PAGE_ADAPTIVE_MODE_BINDING_KEY)
                .unwrap();
            binding.unbind();
        }
        song_page.unbind_player();
        song_page.unbind_song_list();
    } else if let Some(recognized_page) = page.downcast_ref::<RecognizedPage>() {
        unsafe {
            let song_activated_handler_id = recognized_page
                .steal_data::<glib::SignalHandlerId>(RECOGNIZED_PAGE_SONG_ACTIVATED_HANDLER_ID_KEY)
                .unwrap();
            recognized_page.disconnect(song_activated_handler_id);

            let adaptive_mode_binding = recognized_page
                .steal_data::<glib::Binding>(RECOGNIZED_PAGE_ADAPTIVE_MODE_BINDING_KEY)
                .unwrap();
            adaptive_mode_binding.unbind();
        }
        recognized_page.unbind_player();
    } else {
        unreachable!(
            "tried to unbind unknown navigation page type `{}`",
            page.type_()
        );
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

    use crate::{database, RESOURCES_FILE};

    static GRESOURCES_INIT: Once = Once::new();

    fn init_gresources() {
        GRESOURCES_INIT.call_once(|| {
            let res = gio::Resource::load(RESOURCES_FILE).unwrap();
            gio::resources_register(&res);
        });
    }

    fn new_test_song(id: &str) -> Song {
        Song::builder(&Uid::for_test(id), id, id, id).build()
    }

    #[track_caller]
    fn assert_navigation_stack_n_pages(view: &HistoryView, expected_n_pages: u32) {
        assert_eq!(
            view.imp().navigation_view.navigation_stack().n_items(),
            expected_n_pages
        );
    }

    #[track_caller]
    fn assert_forward_navigation_stack_n_pages(view: &HistoryView, expected_n_pages: usize) {
        assert_eq!(
            view.imp().navigation_forward_stack.borrow().len(),
            expected_n_pages
        );
    }

    #[track_caller]
    fn assert_navigation_visible_page_type<T: IsA<adw::NavigationPage>>(view: &HistoryView) {
        assert_eq!(
            view.imp().navigation_view.visible_page().unwrap().type_(),
            T::static_type()
        );
    }

    #[track_caller]
    fn assert_navigation_visible_page_is_song_page_with_id(
        view: &HistoryView,
        expected_song_page_song_id: &str,
    ) {
        assert_eq!(
            view.imp()
                .navigation_view
                .visible_page()
                .expect("visible page must exist")
                .downcast_ref::<SongPage>()
                .expect("visible page must be a `SongPage`")
                .song()
                .expect("song page must have a song")
                .id_ref(),
            &Uid::for_test(expected_song_page_song_id)
        );
    }

    // Returns true if a page has been pushed
    fn simulate_navigate_forward(view: &HistoryView) -> bool {
        let last = view.imp().navigation_forward_stack.borrow().last().cloned();

        if let Some(last) = last {
            view.imp().navigation_view.push(&last);
            true
        } else {
            false
        }
    }

    #[gtk::test]
    fn navigation_simple() {
        init_gresources();
        gst::init().unwrap(); // For Player

        let player = Player::new();
        let (env, _tempdir) = database::new_test_env();
        let song_list = SongList::load_from_env(env).unwrap();

        let song = new_test_song("1");
        song_list.insert(song.clone()).unwrap();

        let view = HistoryView::new();
        view.bind_player(&player);
        view.bind_song_list(&song_list);

        assert_navigation_stack_n_pages(&view, 1);
        assert_forward_navigation_stack_n_pages(&view, 0);
        assert_navigation_visible_page_type::<adw::NavigationPage>(&view);
        assert!(view.is_on_navigation_main_page());

        view.push_song_page(&song);
        assert_navigation_stack_n_pages(&view, 2);
        assert_forward_navigation_stack_n_pages(&view, 0);
        assert_navigation_visible_page_type::<SongPage>(&view);
        assert!(!view.is_on_navigation_main_page());

        view.push_recognized_page(&[]);
        assert_navigation_stack_n_pages(&view, 3);
        assert_forward_navigation_stack_n_pages(&view, 0);
        assert_navigation_visible_page_type::<RecognizedPage>(&view);
        assert!(!view.is_on_navigation_main_page());

        // Already on last page, navigating forward should not do anything
        assert!(!simulate_navigate_forward(&view));
        assert_navigation_stack_n_pages(&view, 3);
        assert_forward_navigation_stack_n_pages(&view, 0);
        assert_navigation_visible_page_type::<RecognizedPage>(&view);
        assert!(!view.is_on_navigation_main_page());

        assert!(view.pop_page());
        assert_navigation_stack_n_pages(&view, 2);
        assert_forward_navigation_stack_n_pages(&view, 1);
        assert_navigation_visible_page_type::<SongPage>(&view);
        assert!(!view.is_on_navigation_main_page());

        assert!(view.pop_page());
        assert_navigation_stack_n_pages(&view, 1);
        assert_forward_navigation_stack_n_pages(&view, 2);
        assert_navigation_visible_page_type::<adw::NavigationPage>(&view);
        assert!(view.is_on_navigation_main_page());

        // Already on main navigation page, popping page should not do anything
        assert!(!view.pop_page());
        assert_navigation_stack_n_pages(&view, 1);
        assert_forward_navigation_stack_n_pages(&view, 2);
        assert_navigation_visible_page_type::<adw::NavigationPage>(&view);
        assert!(view.is_on_navigation_main_page());
    }

    #[gtk::test]
    fn navigation_pop_and_push() {
        init_gresources();
        gst::init().unwrap(); // For Player

        let player = Player::new();
        let (env, _tempdir) = database::new_test_env();
        let song_list = SongList::load_from_env(env).unwrap();

        let song_1 = new_test_song("1");
        let song_2 = new_test_song("2");
        let song_3 = new_test_song("3");
        song_list
            .insert_many(vec![song_1.clone(), song_2.clone(), song_3.clone()])
            .unwrap();

        let view = HistoryView::new();
        view.bind_player(&player);
        view.bind_song_list(&song_list);
        assert_navigation_stack_n_pages(&view, 1);
        assert_forward_navigation_stack_n_pages(&view, 0);

        view.push_song_page(&song_1);
        view.push_song_page(&song_2);
        view.push_recognized_page(&[]);
        assert_navigation_stack_n_pages(&view, 4);
        assert_forward_navigation_stack_n_pages(&view, 0);

        assert!(view.pop_page());
        assert!(view.pop_page());
        assert_navigation_stack_n_pages(&view, 2);
        assert_forward_navigation_stack_n_pages(&view, 2);

        view.push_song_page(&song_3);
        assert_navigation_stack_n_pages(&view, 3);
        assert_forward_navigation_stack_n_pages(&view, 0);

        assert!(!simulate_navigate_forward(&view));
        assert_navigation_stack_n_pages(&view, 3);
        assert_forward_navigation_stack_n_pages(&view, 0);
    }

    #[gtk::test]
    fn navigation_pop_and_forward() {
        init_gresources();
        gst::init().unwrap(); // For Player

        let player = Player::new();
        let (env, _tempdir) = database::new_test_env();
        let song_list = SongList::load_from_env(env).unwrap();

        let song_1 = new_test_song("1");
        let song_2 = new_test_song("2");
        song_list
            .insert_many(vec![song_1.clone(), song_2.clone()])
            .unwrap();

        let view = HistoryView::new();
        view.bind_player(&player);
        view.bind_song_list(&song_list);
        assert_navigation_stack_n_pages(&view, 1);
        assert_forward_navigation_stack_n_pages(&view, 0);
        assert_navigation_visible_page_type::<adw::NavigationPage>(&view);

        view.push_song_page(&song_1);
        view.push_recognized_page(&[]);
        view.push_song_page(&song_2);
        assert_navigation_stack_n_pages(&view, 4);
        assert_forward_navigation_stack_n_pages(&view, 0);
        assert_navigation_visible_page_is_song_page_with_id(&view, "2");

        assert!(view.pop_page());
        assert_navigation_stack_n_pages(&view, 3);
        assert_forward_navigation_stack_n_pages(&view, 1);
        assert_navigation_visible_page_type::<RecognizedPage>(&view);

        assert!(simulate_navigate_forward(&view));
        assert_navigation_stack_n_pages(&view, 4);
        assert_forward_navigation_stack_n_pages(&view, 0);
        assert_navigation_visible_page_is_song_page_with_id(&view, "2");

        assert!(!simulate_navigate_forward(&view));
        assert_navigation_stack_n_pages(&view, 4);
        assert_forward_navigation_stack_n_pages(&view, 0);
        assert_navigation_visible_page_is_song_page_with_id(&view, "2");

        assert!(view.pop_page());
        assert!(view.pop_page());
        assert!(view.pop_page());
        assert_navigation_stack_n_pages(&view, 1);
        assert_forward_navigation_stack_n_pages(&view, 3);
        assert_navigation_visible_page_type::<adw::NavigationPage>(&view);

        assert!(!view.pop_page());
        assert_navigation_stack_n_pages(&view, 1);
        assert_forward_navigation_stack_n_pages(&view, 3);
        assert_navigation_visible_page_type::<adw::NavigationPage>(&view);

        assert!(simulate_navigate_forward(&view));
        assert_navigation_stack_n_pages(&view, 2);
        assert_forward_navigation_stack_n_pages(&view, 2);
        assert_navigation_visible_page_is_song_page_with_id(&view, "1");

        assert!(simulate_navigate_forward(&view));
        assert_navigation_stack_n_pages(&view, 3);
        assert_forward_navigation_stack_n_pages(&view, 1);
        assert_navigation_visible_page_type::<RecognizedPage>(&view);

        assert!(simulate_navigate_forward(&view));
        assert_navigation_stack_n_pages(&view, 4);
        assert_forward_navigation_stack_n_pages(&view, 0);
        assert_navigation_visible_page_is_song_page_with_id(&view, "2");

        assert!(!simulate_navigate_forward(&view));
        assert_navigation_stack_n_pages(&view, 4);
        assert_forward_navigation_stack_n_pages(&view, 0);
        assert_navigation_visible_page_is_song_page_with_id(&view, "2");
    }

    #[gtk::test]
    fn navigation_complex() {
        init_gresources();
        gst::init().unwrap(); // For Player

        let player = Player::new();
        let (env, _tempdir) = database::new_test_env();
        let song_list = SongList::load_from_env(env).unwrap();

        let song_1 = new_test_song("1");
        let song_2 = new_test_song("2");
        let song_3 = new_test_song("3");
        song_list
            .insert_many(vec![song_1.clone(), song_2.clone(), song_3.clone()])
            .unwrap();

        let view = HistoryView::new();
        view.bind_player(&player);
        view.bind_song_list(&song_list);

        view.push_song_page(&song_1);
        view.push_recognized_page(&[]);
        view.push_song_page(&song_2);

        assert!(!simulate_navigate_forward(&view));
        assert_navigation_visible_page_is_song_page_with_id(&view, "2");

        assert!(view.pop_page());
        assert_navigation_visible_page_type::<RecognizedPage>(&view);

        assert!(simulate_navigate_forward(&view));
        assert_navigation_visible_page_is_song_page_with_id(&view, "2");

        assert!(view.pop_page());
        assert_navigation_visible_page_type::<RecognizedPage>(&view);

        assert!(view.pop_page());
        assert_navigation_visible_page_is_song_page_with_id(&view, "1");

        view.push_song_page(&song_3);
        assert_navigation_visible_page_is_song_page_with_id(&view, "3");

        assert!(!simulate_navigate_forward(&view));
        assert_navigation_visible_page_is_song_page_with_id(&view, "3");

        assert!(view.pop_page());
        assert_navigation_visible_page_is_song_page_with_id(&view, "1");

        assert!(view.pop_page());
        assert_navigation_visible_page_type::<adw::NavigationPage>(&view);

        assert!(!view.pop_page());
        assert_navigation_visible_page_type::<adw::NavigationPage>(&view);

        assert!(simulate_navigate_forward(&view));
        assert_navigation_visible_page_is_song_page_with_id(&view, "1");

        assert!(simulate_navigate_forward(&view));
        assert_navigation_visible_page_is_song_page_with_id(&view, "3");

        view.push_song_page(&song_2);
        assert_navigation_visible_page_is_song_page_with_id(&view, "2");

        assert_navigation_stack_n_pages(&view, 4);
        assert_forward_navigation_stack_n_pages(&view, 0);
    }

    #[gtk::test]
    fn navigation_push_song_page_same_song_id() {
        init_gresources();
        gst::init().unwrap(); // For Player

        let player = Player::new();
        let (env, _tempdir) = database::new_test_env();
        let song_list = SongList::load_from_env(env).unwrap();

        let song_1 = new_test_song("1");
        let song_2 = new_test_song("2");
        song_list
            .insert_many(vec![song_1.clone(), song_2.clone()])
            .unwrap();

        let view = HistoryView::new();
        view.bind_player(&player);
        view.bind_song_list(&song_list);
        assert_navigation_stack_n_pages(&view, 1);

        // Added unique song, n pages should increase by 1
        view.push_song_page(&song_1);
        assert_navigation_stack_n_pages(&view, 2);

        // Added unique song, n pages should increase by 1
        view.push_song_page(&song_2);
        assert_navigation_stack_n_pages(&view, 3);

        // Added same song as last, n pages should not change
        view.push_song_page(&song_2);
        assert_navigation_stack_n_pages(&view, 3);

        // Added recognized page, n pages should increase by 1
        view.push_recognized_page(&[]);
        assert_navigation_stack_n_pages(&view, 4);

        // Added same song as last, but there is a recognized page in between so
        // n pages should increase by 1
        view.push_song_page(&song_1);
        assert_navigation_stack_n_pages(&view, 5);

        // Added an already added song, but non adjacent, n pages should increase by 1
        view.push_song_page(&song_2);
        assert_navigation_stack_n_pages(&view, 6);
    }

    #[gtk::test]
    #[should_panic(
        expected = "there must not be already a `RecognizedPage` on the navigation stack"
    )]
    fn navigation_push_recognized_page_with_duplicate() {
        init_gresources();
        gst::init().unwrap(); // For Player

        let player = Player::new();
        let (env, _tempdir) = database::new_test_env();
        let song_list = SongList::load_from_env(env).unwrap();

        let view = HistoryView::new();
        view.bind_player(&player);
        view.bind_song_list(&song_list);

        view.push_recognized_page(&[]);
        view.push_recognized_page(&[]);
    }
}
