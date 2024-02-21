#![warn(
    rust_2018_idioms,
    clippy::items_after_statements,
    clippy::needless_pass_by_value,
    clippy::semicolon_if_nothing_returned,
    clippy::match_wildcard_for_single_variants,
    clippy::inefficient_to_string,
    clippy::map_unwrap_or,
    clippy::implicit_clone,
    clippy::struct_excessive_bools,
    clippy::trivially_copy_pass_by_ref,
    clippy::unreadable_literal,
    clippy::if_not_else,
    clippy::doc_markdown,
    clippy::unused_async,
    clippy::default_trait_access,
    clippy::unnecessary_wraps,
    clippy::unused_self,
    clippy::dbg_macro,
    clippy::todo,
    clippy::map_unwrap_or,
    clippy::or_fun_call,
    clippy::print_stdout
)]

mod about;
mod album_art;
mod application;
mod cancelled;
mod config;
mod database;
mod date_time;
mod device;
mod external_link;
mod external_links;
mod i18n;
mod inspector_page;
mod player;
mod preferences_dialog;
mod recognizer;
mod serde_helpers;
mod settings;
mod song;
mod song_filter;
mod song_list;
mod song_sorter;
mod uid;
mod utils;
mod window;

use gettextrs::{gettext, LocaleCategory};
use gtk::{gio, glib};

use self::application::Application;
use self::config::{GETTEXT_PACKAGE, LOCALEDIR, RESOURCES_FILE};

fn main() -> glib::ExitCode {
    tracing_subscriber::fmt::init();

    gettextrs::setlocale(LocaleCategory::LcAll, "");
    gettextrs::bindtextdomain(GETTEXT_PACKAGE, LOCALEDIR).expect("Unable to bind the text domain");
    gettextrs::textdomain(GETTEXT_PACKAGE).expect("Unable to switch to the text domain");

    glib::set_application_name(&gettext("Mousai"));

    gst::init().expect("Unable to start GStreamer");

    let res = gio::Resource::load(RESOURCES_FILE).expect("Could not load gresource file");
    gio::resources_register(&res);

    let app = Application::new();
    app.run()
}
