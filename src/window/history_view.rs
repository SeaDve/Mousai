use adw::prelude::*;
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
    debug_assert_eq_or_log, debug_assert_or_log, debug_unreachable_or_log,
    model::{Song, SongFilter, SongId, SongList, SongSorter},
    player::Player,
    recognizer::Recognizer,
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

    #[derive(Debug, Default, glib::Properties, gtk::CompositeTemplate)]
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
        pub(super) leaflet: TemplateChild<adw::Leaflet>,
        #[template_child]
        pub(super) history_child: TemplateChild<gtk::Box>,
        #[template_child]
        pub(super) header_bar_stack: TemplateChild<gtk::Stack>,
        #[template_child]
        pub(super) main_header_bar: TemplateChild<gtk::HeaderBar>,
        #[template_child]
        pub(super) recognizer_status: TemplateChild<RecognizerStatus>,
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

        pub(super) player: OnceCell<WeakRef<Player>>,
        pub(super) song_list: OnceCell<WeakRef<SongList>>,
        pub(super) filter_model: OnceCell<WeakRef<gtk::FilterListModel>>,
        pub(super) selection_model: OnceCell<WeakRef<gtk::MultiSelection>>,

        pub(super) songs_purgatory: RefCell<Vec<Song>>,
        pub(super) undo_remove_song_toast: RefCell<Option<adw::Toast>>,

        pub(super) leaflet_pages_purgatory: RefCell<Vec<adw::LeafletPage>>,
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

                debug_assert_or_log!(
                    !selected_songs.is_empty(),
                    "copying should only be allowed if there is atleast one selected"
                );

                let text = selected_songs
                    .iter()
                    .map(|song| song.copy_term())
                    .collect::<Vec<_>>()
                    .join("\n");
                obj.display().clipboard().set_text(&text);

                let toast = adw::Toast::new(&gettext("Copied to clipboard"));
                utils::app_instance().add_toast(toast);
            });

            klass.install_action("history-view.remove-selected-songs", None, |obj, _, _| {
                let selected_songs = obj.snapshot_selected_songs();
                let song_ids = selected_songs
                    .iter()
                    .map(|song| song.id_ref())
                    .collect::<Vec<_>>();
                obj.remove_songs(&song_ids);
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

            self.leaflet.connect_child_transition_running_notify(
                clone!(@weak obj => move |leaflet| {
                    if !leaflet.is_child_transition_running() {
                        obj.purge_purgatory_leaflet_pages();
                    }
                }),
            );

            if tracing::enabled!(tracing::Level::ERROR) {
                self.leaflet
                    .connect_visible_child_notify(clone!(@weak obj => move |leaflet| {
                        let Some(child) = leaflet.visible_child() else {
                            debug_unreachable_or_log!("leaflet has no visible child");
                            return;
                        };

                        let imp = obj.imp();

                        debug_assert_or_log!(
                            !imp.leaflet_pages_purgatory.borrow().contains(&imp.leaflet.page(&child)),
                            "leaflet's visible child is in purgatory"
                        );
                    }));
            }

            self.empty_page.set_icon_name(Some(APP_ID));
            obj.setup_grid();

            obj.update_selection_actions();
            obj.update_selection_mode_ui();

            obj.update_history_stack_visible_child();
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

    pub fn is_on_leaflet_main_page(&self) -> bool {
        let imp = self.imp();
        imp.leaflet.visible_child() == Some(imp.history_child.get().upcast())
    }

    /// Inserts a recognized page for the given songs after the current page and
    /// set it as the visible child.
    pub fn insert_recognized_page(&self, songs: &[Song]) {
        let imp = self.imp();

        debug_assert_or_log!(
            !imp.leaflet
                .pages()
                .iter::<adw::LeafletPage>()
                .map(|page| page.unwrap())
                .filter(|page| !imp.leaflet_pages_purgatory.borrow().contains(page))
                .any(|page| page.is::<RecognizedPage>()),
            "there is already a `RecognizedPage` on the leaflet"
        );

        let recognized_page = RecognizedPage::new();
        recognized_page.bind_player(&self.player());
        recognized_page.bind_songs(songs);

        let song_activated_handler_id =
            recognized_page.connect_song_activated(clone!(@weak self as obj => move |_, song| {
                obj.insert_song_page(song);
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

        self.leaflet_set_as_tail_and_navigate_to(&recognized_page);
    }

    /// Inserts a song page for the given song after the current visible child and
    /// set it as the visible child.
    pub fn insert_song_page(&self, song: &Song) {
        let imp = self.imp();

        // Return if the last widget is a `SongPage` and its song is the same as the given song
        if let Some(page) = imp.leaflet.visible_child().filter(|child| {
            !imp.leaflet_pages_purgatory
                .borrow()
                .contains(&imp.leaflet.page(child))
        }) {
            if let Some(song_page) = page.downcast_ref::<SongPage>() {
                if Some(song.id_ref()) == song_page.song().map(|song| song.id()).as_ref() {
                    return;
                }
            }
        }

        let song_page = SongPage::new();
        song_page.bind_player(&self.player());
        song_page.set_song(song);

        let song_removed_handler_id =
            song_page.connect_song_removed(clone!(@weak self as obj => move |_, song| {
                obj.remove_songs(&[song.id_ref()]);
                obj.show_undo_remove_song_toast();
            }));
        let adaptive_mode_binding = self
            .bind_property("adaptive-mode", &song_page, "adaptive-mode")
            .sync_create()
            .build();

        unsafe {
            song_page.set_data(
                SONG_PAGE_SONG_REMOVED_HANDLER_ID_KEY,
                song_removed_handler_id,
            );
            song_page.set_data(SONG_PAGE_ADAPTIVE_MODE_BINDING_KEY, adaptive_mode_binding);
        }

        self.leaflet_set_as_tail_and_navigate_to(&song_page);

        // User is already aware of the newly recognized song, so unset it.
        song.set_is_newly_heard(false);
    }

    /// Returns true only if the visible child was changed.
    pub fn navigate_back(&self) -> bool {
        self.imp().leaflet.navigate(adw::NavigationDirection::Back)
    }

    /// Returns true only if the visible child was changed.
    pub fn navigate_forward(&self) -> bool {
        self.imp()
            .leaflet
            .navigate(adw::NavigationDirection::Forward)
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

        let filter = SongFilter::new();
        let sorter = SongSorter::new();

        let filter_model = gtk::FilterListModel::new(Some(song_list.clone()), Some(filter.clone()));
        filter_model.connect_items_changed(clone!(@weak self as obj => move |_, _, _, _| {
            obj.update_history_stack_visible_child();
        }));

        imp.search_entry.connect_search_changed(
            clone!(@weak self as obj, @weak filter, @weak sorter => move |search_entry| {
                let text = search_entry.text();
                filter.set_search(text.as_str());
                sorter.set_search(text);
                obj.update_history_stack_visible_child();
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
                        obj.insert_song_page(song);
                    }
                    None => debug_unreachable_or_log!("activated `{}`, but found no song.", index)
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

    /// Must only be called once
    pub fn bind_recognizer(&self, recognizer: &Recognizer) {
        let imp = self.imp();

        imp.recognizer_status.bind_recognizer(recognizer);

        imp.recognizer_status.connect_show_results_requested(
            clone!(@weak self as obj, @weak recognizer => move |_| {
                let Some(history) = obj.imp()
                    .song_list
                    .get()
                    .and_then(|song_list| song_list.upgrade())
                else {
                    debug_unreachable_or_log!("history not found");
                    return;
                };

                let songs = recognizer
                    .take_recognized_saved_recordings()
                    .iter()
                    .filter_map(|recording| match recording.recognize_result().map(|r| r.0) {
                        Some(Ok(ref song)) => Some(song.clone()),
                        Some(Err(ref err)) => {
                            // TODO handle errors
                            debug_assert_or_log!(err.is_permanent());
                            None
                        }
                        None => {
                            debug_unreachable_or_log!("received none recognize result");
                            None
                        }
                    })
                    .collect::<Vec<_>>();

                if songs.is_empty() {
                    tracing::debug!("No saved recordings taken when requested");
                    return;
                }

                for song in &songs {
                    // If the song is not found in the history, set it as newly heard
                    // (That's why an always true value is used after `or`). If it is in the
                    // history and it was newly heard, pass that state to the new value.
                    if history.get(song.id_ref()).map_or(true, |prev| prev.is_newly_heard()) {
                        song.set_is_newly_heard(true);
                    }
                }

                history.append_many(songs.clone());

                obj.insert_recognized_page(&songs);
                obj.scroll_to_top();
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
            .expect("Player was not bound on HistoryView")
            .upgrade()
            .expect("Player was dropped")
    }

    /// This does the following things:
    /// 1. Inserts child after the current visible child, or at the end if there is none.
    /// 2. Add all pages after it to the purgatory.
    /// 3. Set it as the visible child.
    fn leaflet_set_as_tail_and_navigate_to(&self, child: &impl IsA<gtk::Widget>) {
        let imp = self.imp();

        let created_page = if let Some(visible_child) = imp.leaflet.visible_child() {
            imp.leaflet.insert_child_after(child, Some(&visible_child))
        } else {
            imp.leaflet.append(child)
        };

        imp.leaflet
            .pages()
            .iter::<adw::LeafletPage>()
            .map(|page| page.unwrap())
            .filter(|page| !imp.leaflet_pages_purgatory.borrow().contains(page))
            .skip_while(|page| page != &created_page)
            .skip(1)
            .for_each(|page| {
                imp.leaflet_pages_purgatory.borrow_mut().push(page);
            });

        imp.leaflet.set_visible_child(child);
    }

    /// Removes all pages on the purgatory from the leaflet
    fn purge_purgatory_leaflet_pages(&self) {
        let imp = self.imp();

        for page in imp.leaflet_pages_purgatory.take() {
            let child = page.child();

            if let Some(song_page) = child.downcast_ref::<SongPage>() {
                unsafe {
                    let handler_id = song_page
                        .steal_data::<glib::SignalHandlerId>(SONG_PAGE_SONG_REMOVED_HANDLER_ID_KEY)
                        .unwrap();
                    song_page.disconnect(handler_id);

                    let binding = song_page
                        .steal_data::<glib::Binding>(SONG_PAGE_ADAPTIVE_MODE_BINDING_KEY)
                        .unwrap();
                    binding.unbind();
                }
                song_page.unbind_player();
            } else if let Some(recognized_page) = child.downcast_ref::<RecognizedPage>() {
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
                debug_unreachable_or_log!(
                    "tried to purge other leaflet page type `{}`",
                    child.type_()
                );
            }

            imp.leaflet.remove(&child);
        }
    }

    /// Adds songs with ids given to purgatory, and add `SongPage`s that contain them to the purgatory.
    fn remove_songs(&self, song_ids: &[&SongId]) {
        let imp = self.imp();

        if let Some(song_list) = imp
            .song_list
            .get()
            .and_then(|song_list| song_list.upgrade())
        {
            let mut removed_songs = song_list.remove_many(song_ids);
            debug_assert_eq_or_log!(removed_songs.len(), song_ids.len());
            imp.songs_purgatory.borrow_mut().append(&mut removed_songs);
        } else {
            debug_unreachable_or_log!("failed to remove song: SongList not found");
        }

        let leaflet_pages = imp.leaflet.pages();

        // Since the song is removed from history, the `SongPage`s that
        // contain it is dangling, so remove them.
        leaflet_pages
            .iter::<adw::LeafletPage>()
            .map(|page| page.unwrap())
            .filter(|page| {
                page.child()
                    .downcast_ref::<SongPage>()
                    .map_or(false, |song_page| {
                        song_page.song().map_or(false, |song_page_song| {
                            song_ids.contains(&song_page_song.id_ref())
                        })
                    })
                    && !imp.leaflet_pages_purgatory.borrow().contains(page)
            })
            .for_each(|page| {
                imp.leaflet_pages_purgatory.borrow_mut().push(page);
            });

        // TODO Delete the songs from active RecognizedPage too. If every song
        // in a RecognizedPage is removed, the RecognizedPage should be removed from the leaflet too.

        let prev_visible_child_index = imp.leaflet.visible_child().and_then(|child| {
            leaflet_pages
                .iter::<adw::LeafletPage>()
                .position(|page| page.unwrap().child() == child)
        });

        // Ensure that the visible child is not a dangling `SongPage`
        // by making the previous non-dangling page the visible child.
        imp.leaflet.set_visible_child(
            &leaflet_pages
                .iter::<adw::LeafletPage>()
                .map(|page| page.unwrap())
                .rev()
                .skip(
                    // reverse the index
                    prev_visible_child_index
                        .map_or(0, |i| leaflet_pages.n_items() as usize - 1 - i),
                )
                .find(|page| !imp.leaflet_pages_purgatory.borrow().contains(page))
                .map_or_else(
                    || imp.history_child.get().upcast::<gtk::Widget>(),
                    |page| page.child(),
                ),
        );
    }

    fn undo_remove_song(&self) {
        let imp = self.imp();

        if let Some(song_list) = imp
            .song_list
            .get()
            .and_then(|song_list| song_list.upgrade())
        {
            song_list.append_many(imp.songs_purgatory.take());
        } else {
            debug_unreachable_or_log!("song list not found");
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
        } else {
            debug_unreachable_or_log!("selection model not found");
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
        } else {
            debug_unreachable_or_log!("selection model not found");
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
                obj.undo_remove_song();
            }));

            toast.connect_dismissed(clone!(@weak self as obj => move |_| {
                let imp = obj.imp();
                imp.songs_purgatory.borrow_mut().clear();
                imp.undo_remove_song_toast.take();
            }));

            utils::app_instance().add_toast(toast.clone());

            imp.undo_remove_song_toast.replace(Some(toast));
        }

        // Add this point we should already have a toast setup
        if let Some(ref toast) = *imp.undo_remove_song_toast.borrow() {
            let n_removed = imp.songs_purgatory.borrow().len();
            toast.set_title(&ngettext!(
                "Removed {} song",
                "Removed {} songs",
                n_removed as u32,
                n_removed
            ));

            // Reset toast timeout
            utils::app_instance().add_toast(toast.clone());
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

        self.action_set_enabled("history-view.copy-selected-song", selection_size != 0);
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
                if let Some(selection_model) = obj.imp().selection_model.get().and_then(|model| model.upgrade()) {
                    if tile.is_active() {
                        selection_model.select_item(list_item.position(), false);
                    } else {
                        selection_model.unselect_item(list_item.position());
                    }
                } else {
                    debug_unreachable_or_log!("selection model not found");
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
            let res =
                gio::Resource::load(RESOURCES_FILE).expect("Tests could not load gresource file");
            gio::resources_register(&res);
        });
    }

    fn new_test_song(id: &str) -> Song {
        Song::builder(&SongId::for_test(id), id, id, id).build()
    }

    fn trigger_purge_purgatory_leaflet_pages(view: &HistoryView) {
        view.imp().leaflet.notify("child-transition-running");
    }

    #[track_caller]
    fn assert_leaflet_n_pages(view: &HistoryView, expected_n_pages: u32) {
        assert_eq!(view.imp().leaflet.pages().n_items(), expected_n_pages);
    }

    #[track_caller]
    fn assert_leaflet_visible_child_song_id(view: &HistoryView, expected_id: &str) {
        assert_eq!(
            view.imp()
                .leaflet
                .visible_child()
                .unwrap()
                .downcast_ref::<SongPage>()
                .unwrap()
                .song()
                .unwrap()
                .id(),
            SongId::for_test(expected_id)
        );
    }

    #[track_caller]
    fn assert_leaflet_visible_child_type<T: glib::StaticType>(view: &HistoryView) {
        assert!(view.imp().leaflet.visible_child().unwrap().is::<T>());
    }

    #[gtk::test]
    fn page_navigation() {
        init_gresources();
        gst::init().unwrap(); // For Player

        let player = Player::new();
        let (env, _tempdir) = database::new_test_env();
        let song_list = SongList::load_from_env(env).unwrap();

        let song = new_test_song("1");
        song_list.append(song.clone());

        let view = HistoryView::new();
        view.bind_player(&player);
        view.bind_song_list(&song_list);

        assert_leaflet_n_pages(&view, 1);
        assert_leaflet_visible_child_type::<gtk::Box>(&view);
        assert!(view.is_on_leaflet_main_page());

        view.insert_song_page(&song);
        assert_leaflet_n_pages(&view, 2);
        assert_leaflet_visible_child_type::<SongPage>(&view);
        assert!(!view.is_on_leaflet_main_page());

        view.insert_recognized_page(&[]);
        assert_leaflet_n_pages(&view, 3);
        assert_leaflet_visible_child_type::<RecognizedPage>(&view);
        assert!(!view.is_on_leaflet_main_page());

        // Already on last page, navigating forward should not do anything
        assert!(!view.navigate_forward());
        assert_leaflet_n_pages(&view, 3);
        assert_leaflet_visible_child_type::<RecognizedPage>(&view);
        assert!(!view.is_on_leaflet_main_page());

        assert!(view.navigate_back());
        assert_leaflet_n_pages(&view, 3);
        assert_leaflet_visible_child_type::<SongPage>(&view);
        assert!(!view.is_on_leaflet_main_page());

        assert!(view.navigate_back());
        assert_leaflet_n_pages(&view, 3);
        assert_leaflet_visible_child_type::<gtk::Box>(&view);
        assert!(view.is_on_leaflet_main_page());

        // Already on first page, navigating backward should not do anything
        assert!(!view.navigate_back());
        assert_leaflet_n_pages(&view, 3);
        assert_leaflet_visible_child_type::<gtk::Box>(&view);
        assert!(view.is_on_leaflet_main_page());
    }

    #[gtk::test]
    fn page_insertion() {
        init_gresources();
        gst::init().unwrap(); // For Player

        let player = Player::new();
        let (env, _tempdir) = database::new_test_env();
        let song_list = SongList::load_from_env(env).unwrap();

        let song_1 = new_test_song("1");
        let song_2 = new_test_song("2");
        song_list.append_many(vec![song_1.clone(), song_2.clone()]);

        let view = HistoryView::new();
        view.bind_player(&player);
        view.bind_song_list(&song_list);
        assert_leaflet_n_pages(&view, 1);

        // Added unique song, n pages should increase by 1
        view.insert_song_page(&song_1);
        assert_leaflet_n_pages(&view, 2);

        // Added unique song, n pages should increase by 1
        view.insert_song_page(&song_2);
        assert_leaflet_n_pages(&view, 3);

        // Added same song as last, n pages should not change
        view.insert_song_page(&song_2);
        assert_leaflet_n_pages(&view, 3);

        // Added recognized page, n pages should increase by 1
        view.insert_recognized_page(&[]);
        assert_leaflet_n_pages(&view, 4);

        // Added same song as last, but there is a recognized page in between so
        // n pages should increase by 1
        view.insert_song_page(&song_1);
        assert_leaflet_n_pages(&view, 5);

        // Added an already added song, but non adjacent, n pages should increase by 1
        view.insert_song_page(&song_2);
        assert_leaflet_n_pages(&view, 6);
    }

    #[gtk::test]
    fn page_navigate_back_then_insert() {
        init_gresources();
        gst::init().unwrap(); // For Player

        let player = Player::new();
        let (env, _tempdir) = database::new_test_env();
        let song_list = SongList::load_from_env(env).unwrap();

        let song_1 = new_test_song("1");
        let song_2 = new_test_song("2");
        let song_3 = new_test_song("3");
        song_list.append_many(vec![song_1.clone(), song_2.clone(), song_3.clone()]);

        let view = HistoryView::new();
        view.bind_player(&player);
        view.bind_song_list(&song_list);
        assert_leaflet_n_pages(&view, 1);

        view.insert_song_page(&song_1);
        view.insert_song_page(&song_2);
        view.insert_recognized_page(&[]);
        assert_leaflet_n_pages(&view, 4);

        view.navigate_back();
        view.navigate_back();
        assert_leaflet_n_pages(&view, 4);

        // Added song after navigating back to second page with 2 tail page,
        // 2 tail pages should be removed and n pages should decrease to 3
        // (main page + second song page + added song page)
        view.insert_song_page(&song_3);
        trigger_purge_purgatory_leaflet_pages(&view);
        assert_leaflet_n_pages(&view, 3);

        // Navigating back should not do anything since the mentioned tail
        // pages are already removed
        assert!(!view.navigate_forward());
    }

    #[gtk::test]
    fn remove_song() {
        init_gresources();
        gst::init().unwrap(); // For Player

        let player = Player::new();
        let (env, _tempdir) = database::new_test_env();
        let song_list = SongList::load_from_env(env).unwrap();

        let song_1 = new_test_song("1");
        let song_2 = new_test_song("2");
        let song_3 = new_test_song("3");
        song_list.append_many(vec![song_1.clone(), song_2.clone(), song_3.clone()]);

        let view = HistoryView::new();
        view.bind_player(&player);
        view.bind_song_list(&song_list);
        assert_leaflet_n_pages(&view, 1);

        view.insert_song_page(&song_1);
        assert_leaflet_n_pages(&view, 2);
        assert_leaflet_visible_child_song_id(&view, "1");

        view.insert_song_page(&song_2);
        assert_leaflet_n_pages(&view, 3);
        assert_leaflet_visible_child_song_id(&view, "2");

        view.insert_song_page(&song_3);
        assert_leaflet_n_pages(&view, 4);
        assert_leaflet_visible_child_song_id(&view, "3");

        view.insert_song_page(&song_1);
        assert_leaflet_n_pages(&view, 5);
        assert_leaflet_visible_child_song_id(&view, "1");

        // Since song_1 is added twice non-adjacently, it should reduce
        // the number of pages by 2
        view.remove_songs(&[song_1.id_ref()]);
        trigger_purge_purgatory_leaflet_pages(&view);
        assert_leaflet_n_pages(&view, 3);

        assert_leaflet_visible_child_song_id(&view, "3");

        // Since song_2 is added once, it should reduce the number of pages by 1
        view.remove_songs(&[song_2.id_ref()]);
        trigger_purge_purgatory_leaflet_pages(&view);
        assert_leaflet_n_pages(&view, 2);

        assert_leaflet_visible_child_song_id(&view, "3");

        view.remove_songs(&[song_3.id_ref()]);
        trigger_purge_purgatory_leaflet_pages(&view);
        assert_leaflet_n_pages(&view, 1);
    }

    #[gtk::test]
    fn remove_song_many() {
        init_gresources();
        gst::init().unwrap(); // For Player

        let player = Player::new();
        let (env, _tempdir) = database::new_test_env();
        let song_list = SongList::load_from_env(env).unwrap();

        let song_1 = new_test_song("1");
        let song_2 = new_test_song("2");
        let song_3 = new_test_song("3");
        song_list.append_many(vec![song_1.clone(), song_2.clone(), song_3.clone()]);

        let view = HistoryView::new();
        view.bind_player(&player);
        view.bind_song_list(&song_list);
        assert_leaflet_n_pages(&view, 1);

        view.insert_song_page(&song_1);
        view.insert_song_page(&song_2);
        view.insert_song_page(&song_3);
        view.insert_song_page(&song_1);
        view.insert_song_page(&song_2);
        assert_leaflet_n_pages(&view, 6);

        view.remove_songs(&[song_2.id_ref(), song_3.id_ref()]);
        trigger_purge_purgatory_leaflet_pages(&view);
        assert_leaflet_visible_child_song_id(&view, "1");
        assert_leaflet_n_pages(&view, 3);
    }

    #[gtk::test]
    fn remove_song_middle_visible_child() {
        init_gresources();
        gst::init().unwrap(); // For Player

        let player = Player::new();
        let (env, _tempdir) = database::new_test_env();
        let song_list = SongList::load_from_env(env).unwrap();

        let song_1 = new_test_song("1");
        let song_2 = new_test_song("2");
        let song_3 = new_test_song("3");
        song_list.append_many(vec![song_1.clone(), song_2.clone(), song_3.clone()]);

        let view = HistoryView::new();
        view.bind_player(&player);
        view.bind_song_list(&song_list);

        view.insert_song_page(&song_1);
        view.insert_song_page(&song_2);
        view.insert_song_page(&song_3);
        assert_leaflet_n_pages(&view, 4);

        assert_leaflet_visible_child_song_id(&view, "3");

        view.navigate_back();
        assert_leaflet_visible_child_song_id(&view, "2");

        view.remove_songs(&[song_2.id_ref()]);
        trigger_purge_purgatory_leaflet_pages(&view);
        assert_leaflet_n_pages(&view, 3);
        assert_leaflet_visible_child_song_id(&view, "1");

        view.navigate_forward();
        assert_leaflet_visible_child_song_id(&view, "3");
    }

    #[gtk::test]
    fn remove_song_many_middle_visible_child() {
        init_gresources();
        gst::init().unwrap(); // For Player

        let player = Player::new();
        let (env, _tempdir) = database::new_test_env();
        let song_list = SongList::load_from_env(env).unwrap();

        let song_1 = new_test_song("1");
        let song_2 = new_test_song("2");
        let song_3 = new_test_song("3");
        let song_4 = new_test_song("4");
        song_list.append_many(vec![
            song_1.clone(),
            song_2.clone(),
            song_3.clone(),
            song_4.clone(),
        ]);

        let view = HistoryView::new();
        view.bind_player(&player);
        view.bind_song_list(&song_list);

        view.insert_song_page(&song_1);
        view.insert_song_page(&song_2);
        view.insert_song_page(&song_3);
        view.insert_song_page(&song_4);
        assert_leaflet_n_pages(&view, 5);

        assert_leaflet_visible_child_song_id(&view, "4");

        view.navigate_back();
        view.navigate_back();
        assert_leaflet_visible_child_song_id(&view, "2");

        view.remove_songs(&[song_2.id_ref(), song_3.id_ref()]);
        trigger_purge_purgatory_leaflet_pages(&view);
        assert_leaflet_n_pages(&view, 3);
        assert_leaflet_visible_child_song_id(&view, "1");

        view.navigate_forward();
        assert_leaflet_visible_child_song_id(&view, "4");
    }

    #[gtk::test]
    fn remove_song_middle_visible_child_with_recognized_page() {
        init_gresources();
        gst::init().unwrap(); // For Player

        let player = Player::new();
        let (env, _tempdir) = database::new_test_env();
        let song_list = SongList::load_from_env(env).unwrap();

        let song_1 = new_test_song("1");
        let song_2 = new_test_song("2");
        song_list.append_many(vec![song_1.clone(), song_2.clone()]);

        let view = HistoryView::new();
        view.bind_player(&player);
        view.bind_song_list(&song_list);

        view.insert_recognized_page(&[]);
        view.insert_song_page(&song_1);
        view.insert_song_page(&song_2);
        assert_leaflet_n_pages(&view, 4);

        assert_leaflet_visible_child_song_id(&view, "2");

        view.navigate_back();
        assert_leaflet_visible_child_song_id(&view, "1");

        view.remove_songs(&[song_1.id_ref()]);
        trigger_purge_purgatory_leaflet_pages(&view);
        assert_leaflet_n_pages(&view, 3);
        assert_leaflet_visible_child_type::<RecognizedPage>(&view);

        view.navigate_forward();
        assert_leaflet_visible_child_song_id(&view, "2");
    }

    #[gtk::test]
    fn remove_song_many_middle_visible_child_with_recognized_page() {
        init_gresources();
        gst::init().unwrap(); // For Player

        let player = Player::new();
        let (env, _tempdir) = database::new_test_env();
        let song_list = SongList::load_from_env(env).unwrap();

        let song_1 = new_test_song("1");
        let song_2 = new_test_song("2");
        let song_3 = new_test_song("3");
        song_list.append_many(vec![song_1.clone(), song_2.clone(), song_3.clone()]);

        let view = HistoryView::new();
        view.bind_player(&player);
        view.bind_song_list(&song_list);

        view.insert_song_page(&song_1);
        view.insert_recognized_page(&[]);
        view.insert_song_page(&song_2);
        view.insert_song_page(&song_3);
        assert_leaflet_n_pages(&view, 5);

        assert_leaflet_visible_child_song_id(&view, "3");

        view.navigate_back();
        assert_leaflet_visible_child_song_id(&view, "2");

        view.remove_songs(&[song_1.id_ref(), song_2.id_ref()]);
        trigger_purge_purgatory_leaflet_pages(&view);
        assert_leaflet_n_pages(&view, 3);
        assert_leaflet_visible_child_type::<RecognizedPage>(&view);

        view.navigate_forward();
        assert_leaflet_visible_child_song_id(&view, "3");
    }
}
