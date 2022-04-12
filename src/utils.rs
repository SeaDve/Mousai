use futures_channel::oneshot;
use gtk::{
    gio::{self, prelude::*},
    glib,
};

use std::{cell::Cell, path::Path, rc::Rc, time::Duration};

use crate::core::{Cancellable, Cancelled};

/// Spawns a future in the main context
#[macro_export]
macro_rules! spawn {
    ($future:expr) => {
        let ctx = glib::MainContext::default();
        ctx.spawn_local($future);
    };
    ($priority:expr, $future:expr) => {
        let ctx = glib::MainContext::default();
        ctx.spawn_local_with_priority($priority, $future);
    };
}

/// Send something to a [`glib::Sender`](glib::Sender)
#[macro_export]
macro_rules! send {
    ($sender:expr, $action:expr) => {
        if let Err(err) = $sender.send($action) {
            log::error!("Failed to send \"{}\" action: {err:?}", stringify!($action));
        }
    };
}

pub async fn timeout_future(
    interval: Duration,
    cancellable: &Cancellable,
) -> Result<(), Cancelled> {
    let (sender, receiver) = oneshot::channel();
    let is_source_removed = Rc::new(Cell::new(false));
    let is_source_removed_clone = Rc::clone(&is_source_removed);

    let source_id = glib::timeout_add_local_once(interval, move || {
        let _ = sender.send(());
        is_source_removed.set(true);
    });

    cancellable.connect_cancelled(move |_| {
        if !is_source_removed_clone.get() {
            // Once source is removed the sender that was moved in the closure
            // will be dropped. Causing the receiver to emit `oneshot::Canceled`.
            source_id.remove();
        }
    });

    receiver.await.map_err(|_| Cancelled::default())
}

pub async fn file_to_base64(path: impl AsRef<Path>) -> Result<String, glib::Error> {
    let file = gio::File::for_path(path.as_ref());
    let (contents, _) = file.load_contents_future().await?;
    Ok(base64::encode(contents))
}
