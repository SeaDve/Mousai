use gtk::glib;

use std::future::Future;

/// Spawns a future in the default [`glib::MainContext`]
pub fn spawn<F: Future<Output = ()> + 'static>(fut: F) {
    let ctx = glib::MainContext::default();
    ctx.spawn_local(fut);
}
