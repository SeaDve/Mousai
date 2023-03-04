use adw::{prelude::*, subclass::prelude::*};
use gettextrs::gettext;
use gtk::glib;
use once_cell::unsync::OnceCell;

const OK_RESPONSE_ID: &str = "ok";

mod imp {
    use super::*;

    #[derive(Debug, Default, glib::Properties, gtk::CompositeTemplate)]
    #[properties(wrapper_type = super::ErrorDialog)]
    #[template(resource = "/io/github/seadve/Mousai/ui/error-dialog.ui")]
    pub struct ErrorDialog {
        #[property(get, set, construct_only)]
        pub(super) detailed_error: OnceCell<String>,

        #[template_child]
        pub(super) copy_detailed_error_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub(super) detailed_error_box: TemplateChild<gtk::ListBox>,
        #[template_child]
        pub(super) detailed_error_buffer: TemplateChild<gtk::TextBuffer>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for ErrorDialog {
        const NAME: &'static str = "MsaiErrorDialog";
        type Type = super::ErrorDialog;
        type ParentType = adw::MessageDialog;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for ErrorDialog {
        crate::derived_properties!();

        fn constructed(&self) {
            self.parent_constructed();

            let obj = self.obj();
            obj.add_response(OK_RESPONSE_ID, &gettext("Ok"));
            obj.set_default_response(Some(OK_RESPONSE_ID));

            let detailed_error = obj.detailed_error();

            let has_detailed_error = !detailed_error.is_empty();
            self.detailed_error_box.set_visible(has_detailed_error);

            if has_detailed_error {
                self.detailed_error_buffer.set_text(&detailed_error);
                self.copy_detailed_error_button
                    .connect_clicked(move |button| {
                        button.display().clipboard().set_text(&detailed_error);
                        button.set_tooltip_text(Some(&gettext("Copied to Clipboard")));
                        button.set_icon_name("checkmark-symbolic");
                    });
            }
        }

        fn dispose(&self) {
            self.dispose_template();
        }
    }

    impl WidgetImpl for ErrorDialog {}
    impl WindowImpl for ErrorDialog {}
    impl MessageDialogImpl for ErrorDialog {}
}

glib::wrapper! {
     pub struct ErrorDialog(ObjectSubclass<imp::ErrorDialog>)
        @extends gtk::Widget, gtk::Window, adw::MessageDialog;
}

impl ErrorDialog {
    pub fn new(title: &str, body: &str, detailed_error: &str) -> Self {
        glib::Object::builder()
            .property("heading", title)
            .property("body", body)
            .property("detailed-error", detailed_error)
            .build()
    }
}
