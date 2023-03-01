use gtk::{
    gio,
    glib::{self, prelude::*},
};

use std::future::Future;

use crate::{debug_assert_or_log, Application};

/// Spawns a future in the default [`glib::MainContext`]
pub fn spawn<R, F>(fut: F) -> glib::JoinHandle<R>
where
    R: 'static,
    F: Future<Output = R> + 'static,
{
    let ctx = glib::MainContext::default();
    ctx.spawn_local(fut)
}

/// Get the global instance of `Application`.
///
/// # Panics
/// Panics if the application is not running or if this is
/// called on a non-main thread.
pub fn app_instance() -> Application {
    debug_assert_or_log!(
        gtk::is_initialized_main_thread(),
        "application can only be accessed in the main thread"
    );

    gio::Application::default().unwrap().downcast().unwrap()
}

/// Generate a random "unique" String made up of real time and a random u32 in hex.
pub fn generate_unique_id() -> String {
    format!("{}-{:x}", glib::real_time(), glib::random_int())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unique_generated_id() {
        for i in 0..1000 {
            assert_ne!(
                generate_unique_id(),
                generate_unique_id(),
                "generated ids are equal after {} iterations",
                i
            );
        }
    }
}
