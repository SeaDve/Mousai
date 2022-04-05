use adw::prelude::*;
use gtk::{
    glib::{self, clone},
    subclass::prelude::*,
};
use once_cell::unsync::OnceCell;

use super::{song_cell::SongCell, song_page::SongPage, Window};
use crate::model::{Song, SongList};

mod imp {
    use super::*;
    use glib::WeakRef;
    use gtk::CompositeTemplate;

    #[derive(Debug, Default, CompositeTemplate)]
    #[template(resource = "/io/github/seadve/Mousai/ui/history-view.ui")]
    pub struct HistoryView {
        #[template_child]
        pub stack: TemplateChild<gtk::Stack>,
        #[template_child]
        pub history_stack: TemplateChild<gtk::Stack>,
        #[template_child]
        pub main_page: TemplateChild<gtk::Box>,
        #[template_child]
        pub search_bar: TemplateChild<gtk::SearchBar>,
        #[template_child]
        pub search_entry: TemplateChild<gtk::SearchEntry>,
        #[template_child]
        pub grid: TemplateChild<gtk::GridView>,
        #[template_child]
        pub empty_page: TemplateChild<gtk::Box>,
        #[template_child]
        pub song_page: TemplateChild<SongPage>,

        pub song_list: OnceCell<WeakRef<SongList>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for HistoryView {
        const NAME: &'static str = "MsaiHistoryView";
        type Type = super::HistoryView;
        type ParentType = gtk::Widget;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for HistoryView {
        fn constructed(&self, obj: &Self::Type) {
            self.parent_constructed(obj);

            obj.add_css_class("view");

            obj.setup_grid();

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

    pub fn search_bar(&self) -> gtk::SearchBar {
        self.imp().search_bar.get()
    }

    pub fn show_history(&self) {
        let imp = self.imp();
        self.update_history_stack();
        imp.stack.set_visible_child(&imp.history_stack.get());
    }

    pub fn show_song(&self, song: &Song) {
        let imp = self.imp();
        imp.song_page.set_song(Some(song.clone()));
        imp.stack.set_visible_child(&imp.song_page.get());
    }

    // Must only be called once
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

        let grid = imp.grid.get();
        grid.set_model(Some(&selection_model));
        grid.connect_activate(clone!(@weak self as obj => move |_, index| {
            match selection_model.item(index).and_then(|song| song.downcast::<Song>().ok()) {
                Some(ref song) => obj.show_song(song),
                None => log::error!("Activated `{index}`, but found no song.")
            }
        }));

        imp.song_list.set(song_list.downgrade()).unwrap();

        self.update_history_stack();
    }

    fn update_history_stack(&self) {
        let imp = self.imp();
        let is_model_empty = imp
            .song_list
            .get()
            .and_then(|song_list| song_list.upgrade())
            .map_or_else(|| true, |model| model.n_items() == 0);

        if is_model_empty {
            imp.history_stack.set_visible_child(&imp.empty_page.get());
        } else {
            imp.history_stack.set_visible_child(&imp.main_page.get());
        }
    }

    fn setup_grid(&self) {
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
                log::error!("Cannot bind SongCell to AudioPlayerWidget: HistoryView doesn't have root");
            }
        }));
        factory.connect_unbind(|_, list_item| {
            let song_cell: SongCell = list_item
                .child()
                .and_then(|widget| widget.downcast().ok())
                .expect("HistoryView list item should have a child of SongCell");
            song_cell.unbind();
        });

        let grid = self.imp().grid.get();
        grid.set_factory(Some(&factory));
    }
}

impl Default for HistoryView {
    fn default() -> Self {
        Self::new()
    }
}
