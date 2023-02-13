use adw::{prelude::*, subclass::prelude::*};
use anyhow::{Error, Result};
use gettextrs::gettext;
use gtk::{gio, glib};

use crate::{
    about,
    config::{APP_ID, PKGDATADIR, PROFILE, VERSION},
    core::{AlbumArtStore, Help},
    inspector_page::InspectorPage,
    settings::Settings,
    window::Window,
};

mod imp {
    use super::*;
    use glib::WeakRef;
    use once_cell::unsync::OnceCell;

    #[derive(Debug, Default)]
    pub struct Application {
        pub(super) window: OnceCell<WeakRef<Window>>,
        pub(super) session: OnceCell<soup::Session>,
        pub(super) album_art_store: OnceCell<AlbumArtStore>,
        pub(super) settings: Settings,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Application {
        const NAME: &'static str = "MsaiApplication";
        type Type = super::Application;
        type ParentType = adw::Application;
    }

    impl ObjectImpl for Application {}

    impl ApplicationImpl for Application {
        fn activate(&self) {
            self.parent_activate();

            if let Some(window) = self.obj().main_window() {
                window.present();
            }
        }

        fn startup(&self) {
            self.parent_startup();

            gtk::Window::set_default_icon_name(APP_ID);

            let obj = self.obj();

            obj.setup_gactions();
            obj.setup_accels();

            setup_inspector_page();
        }
    }

    impl GtkApplicationImpl for Application {}
    impl AdwApplicationImpl for Application {}
}

glib::wrapper! {
    pub struct Application(ObjectSubclass<imp::Application>)
        @extends gio::Application, gtk::Application, adw::Application,
        @implements gio::ActionMap, gio::ActionGroup;
}

impl Application {
    pub fn new() -> Self {
        glib::Object::builder()
            .property("application-id", APP_ID)
            .property("resource-base-path", "/io/github/seadve/Mousai/")
            .build()
    }

    pub fn settings(&self) -> Settings {
        self.imp().settings.clone()
    }

    pub fn session(&self) -> &soup::Session {
        self.imp().session.get_or_init(soup::Session::new)
    }

    pub fn album_art_store(&self) -> Result<&AlbumArtStore> {
        self.imp()
            .album_art_store
            .get_or_try_init(|| AlbumArtStore::new(self.session()))
    }

    pub fn present_error(&self, err: &Error) {
        present_error(err, self.main_window().as_ref());
    }

    pub fn add_toast_error(&self, err: &Error) {
        let toast = adw::Toast::builder()
            .title(glib::markup_escape_text(&err.to_string()))
            .priority(adw::ToastPriority::High)
            .build();
        self.add_toast(toast);
    }

    pub fn add_toast(&self, toast: adw::Toast) {
        if let Some(window) = self.main_window() {
            window.add_toast(toast);
        } else {
            tracing::warn!("Failed to add toast: MainWindow doesn't exist");
        }
    }

    pub fn run(&self) -> glib::ExitCode {
        tracing::info!("Mousai ({})", APP_ID);
        tracing::info!("Version: {} ({})", VERSION, PROFILE);
        tracing::info!("Datadir: {}", PKGDATADIR);

        ApplicationExtManual::run(self)
    }

    fn main_window(&self) -> Option<Window> {
        let main_window = self
            .imp()
            .window
            .get_or_init(|| Window::new(self).downgrade())
            .upgrade();

        if main_window.is_none() {
            tracing::warn!("Failed to upgrade WeakRef<Window>");
        }

        main_window
    }

    fn setup_gactions(&self) {
        let quit_action = gio::ActionEntry::builder("quit")
            .activate(|obj: &Self, _, _| {
                if let Some(ref main_window) = obj.main_window() {
                    main_window.close();
                }
                obj.quit();
            })
            .build();
        let about_action = gio::ActionEntry::builder("about")
            .activate(|obj: &Self, _, _| {
                about::present_window(obj.main_window().as_ref());
            })
            .build();
        self.add_action_entries([quit_action, about_action]);
    }

    fn setup_accels(&self) {
        self.set_accels_for_action("app.quit", &["<Control>q"]);
        self.set_accels_for_action("window.close", &["<Control>w"]);
        self.set_accels_for_action("win.navigate-back", &["<Alt>Left", "Escape"]);
        self.set_accels_for_action("win.toggle-playback", &["<Control>space"]);
        self.set_accels_for_action("win.toggle-listen", &["<Control>r"]);
        self.set_accels_for_action("win.toggle-search", &["<Control>f"]);
    }
}

impl Default for Application {
    fn default() -> Self {
        Self::new()
    }
}

fn setup_inspector_page() {
    if gio::IOExtensionPoint::lookup("gtk-inspector-page").is_some() {
        gio::IOExtensionPoint::implement(
            "gtk-inspector-page",
            InspectorPage::static_type(),
            APP_ID,
            10,
        );
    } else {
        tracing::warn!("Failed to setup Mousai's inspector page. IOExtensionPoint `gtk-inspector-page` is likely not found.");
    }
}

fn present_error(err: &Error, transient_for: Option<&impl IsA<gtk::Window>>) {
    let err_text = format!("{:?}", err);

    let err_view = gtk::TextView::builder()
        .buffer(&gtk::TextBuffer::builder().text(&err_text).build())
        .editable(false)
        .monospace(true)
        .top_margin(6)
        .bottom_margin(6)
        .left_margin(6)
        .right_margin(6)
        .build();

    let scrolled_window = gtk::ScrolledWindow::builder()
        .child(&err_view)
        .min_content_height(120)
        .min_content_width(360)
        .build();

    let scrolled_window_row = gtk::ListBoxRow::builder()
        .child(&scrolled_window)
        .overflow(gtk::Overflow::Hidden)
        .activatable(false)
        .selectable(false)
        .build();
    scrolled_window_row.add_css_class("error-view");

    let copy_button = gtk::Button::builder()
        .tooltip_text(gettext("Copy to clipboard"))
        .icon_name("edit-copy-symbolic")
        .valign(gtk::Align::Center)
        .build();
    copy_button.connect_clicked(move |button| {
        button.display().clipboard().set_text(&err_text);
        button.set_tooltip_text(Some(&gettext("Copied to clipboard")));
        button.set_icon_name("checkmark-symbolic");
        button.add_css_class("copy-done");
    });

    let expander = adw::ExpanderRow::builder()
        .title(gettext("Show detailed error"))
        .activatable(false)
        .build();
    expander.add_row(&scrolled_window_row);
    expander.add_action(&copy_button);

    let list_box = gtk::ListBox::builder()
        .selection_mode(gtk::SelectionMode::None)
        .build();
    list_box.add_css_class("boxed-list");
    list_box.append(&expander);

    let err_dialog = adw::MessageDialog::builder()
        .heading(err.to_string())
        .body_use_markup(true)
        .default_response("ok")
        .modal(true)
        .extra_child(&list_box)
        .build();
    err_dialog.set_transient_for(transient_for);

    if let Some(ref help) = err.downcast_ref::<Help>() {
        err_dialog.set_body(&format!("<b>{}</b>: {}", gettext("Help"), help));
    }

    err_dialog.add_response("ok", &gettext("Ok"));
    err_dialog.present();
}
