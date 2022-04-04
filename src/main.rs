#![warn(clippy::doc_markdown)]
#![warn(clippy::missing_const_for_fn)]
#![warn(clippy::or_fun_call)]
#![warn(clippy::needless_pass_by_value)]
#![warn(clippy::explicit_iter_loop)]
#![warn(clippy::semicolon_if_nothing_returned)]
#![warn(clippy::match_wildcard_for_single_variants)]
#![warn(clippy::inefficient_to_string)]
#![warn(clippy::await_holding_refcell_ref)]
#![warn(clippy::map_unwrap_or)]
#![warn(clippy::implicit_clone)]
#![warn(clippy::struct_excessive_bools)]
#![warn(clippy::trivially_copy_pass_by_ref)]
#![warn(clippy::option_if_let_else)]
#![warn(clippy::unreadable_literal)]
#![warn(clippy::if_not_else)]
#![warn(clippy::doc_markdown)]

mod album_art_manager;
mod application;
mod config;
mod core;
mod inspector_page;
mod model;
mod provider;
mod recognizer;
mod song_player;
mod utils;
mod window;

use gettextrs::{gettext, LocaleCategory};
use gtk::{
    gio,
    glib::{self, prelude::*},
};
use once_cell::sync::Lazy;

use self::application::Application;
use self::config::{GETTEXT_PACKAGE, LOCALEDIR, RESOURCES_FILE};
use self::inspector_page::InspectorPage;

static RUNTIME: Lazy<tokio::runtime::Runtime> =
    Lazy::new(|| tokio::runtime::Runtime::new().unwrap());

fn main() {
    pretty_env_logger::init();

    gettextrs::setlocale(LocaleCategory::LcAll, "");
    gettextrs::bindtextdomain(GETTEXT_PACKAGE, LOCALEDIR).expect("Unable to bind the text domain");
    gettextrs::textdomain(GETTEXT_PACKAGE).expect("Unable to switch to the text domain");

    glib::set_application_name(&gettext("Mousai"));

    gtk::init().expect("Unable to start GTK4");
    gst::init().expect("Unable to start GStreamer");

    let res = gio::Resource::load(RESOURCES_FILE).expect("Could not load gresource file");
    gio::resources_register(&res);

    if gio::IOExtensionPoint::lookup("gtk-inspector-page").is_some() {
        gio::IOExtensionPoint::implement(
            "gtk-inspector-page",
            InspectorPage::static_type(),
            "Mousai",
            10,
        );
    }

    let app = Application::new();
    app.run();
}
