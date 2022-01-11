use gtk::{glib, prelude::*, subclass::prelude::*};

use std::cell::RefCell;

use crate::model::{Song, SongList};

mod imp {
    use super::*;
    use gtk::CompositeTemplate;
    use once_cell::sync::Lazy;

    #[derive(Debug, Default, CompositeTemplate)]
    #[template(resource = "/io/github/seadve/Mousai/ui/history-view.ui")]
    pub struct HistoryView {
        #[template_child]
        pub list_box: TemplateChild<gtk::ListBox>,

        pub model: RefCell<Option<SongList>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for HistoryView {
        const NAME: &'static str = "MsaiHistoryView";
        type Type = super::HistoryView;
        type ParentType = gtk::Widget;

        fn class_init(klass: &mut Self::Class) {
            Self::bind_template(klass);
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for HistoryView {
        fn properties() -> &'static [glib::ParamSpec] {
            static PROPERTIES: Lazy<Vec<glib::ParamSpec>> = Lazy::new(|| {
                vec![glib::ParamSpec::new_object(
                    "model",
                    "Model",
                    "Model represented by Self",
                    SongList::static_type(),
                    glib::ParamFlags::READWRITE | glib::ParamFlags::EXPLICIT_NOTIFY,
                )]
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
                "model" => {
                    let model: Option<SongList> = value.get().unwrap();
                    obj.set_model(model.as_ref());
                }
                _ => unimplemented!(),
            }
        }

        fn property(&self, obj: &Self::Type, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
            match pspec.name() {
                "model" => obj.model().to_value(),
                _ => unimplemented!(),
            }
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

    pub fn set_model(&self, model: Option<&SongList>) {
        let imp = imp::HistoryView::from_instance(self);

        imp.list_box.bind_model(model, |data| {
            let song = data.downcast_ref::<Song>().unwrap();
            Self::create_widget_func(song)
        });

        imp.model.replace(model.cloned());
    }

    pub fn model(&self) -> Option<SongList> {
        let imp = imp::HistoryView::from_instance(self);
        imp.model.borrow().clone()
    }

    fn create_widget_func(song: &Song) -> gtk::Widget {
        let row = adw::ExpanderRow::new();

        song.bind_property("title", &row, "title")
            .flags(glib::BindingFlags::SYNC_CREATE)
            .build();

        song.bind_property("artist", &row, "subtitle")
            .flags(glib::BindingFlags::SYNC_CREATE)
            .build();

        row.upcast()
    }
}
