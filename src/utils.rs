use gtk::gio::{self, prelude::*};

use std::path::Path;

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

pub async fn file_to_base64(path: impl AsRef<Path>) -> anyhow::Result<String> {
    let file = gio::File::for_path(path.as_ref());
    let (contents, _) = file.load_contents_future().await?;
    Ok(base64::encode(contents))
}
