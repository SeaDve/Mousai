use gtk::{
    gio,
    glib::{self, prelude::*},
};

use std::future::Future;

use crate::Application;

/// Spawns a future in the default [`glib::MainContext`]
pub fn spawn<F: Future<Output = ()> + 'static>(fut: F) {
    let ctx = glib::MainContext::default();
    ctx.spawn_local(fut);
}

/// Get the global instance of `Application`.
///
/// # Panics
/// Panics if the application is not running or if this is
/// called on a non-main thread.
pub fn app_instance() -> Application {
    debug_assert!(
        gtk::is_initialized_main_thread(),
        "Application can only be accessed in the main thread"
    );

    gio::Application::default().unwrap().downcast().unwrap()
}
