use gtk::gio::{self, prelude::*};

use std::path::Path;

pub async fn file_to_base64(path: impl AsRef<Path>) -> anyhow::Result<String> {
    let file = gio::File::for_path(path.as_ref());
    let (contents, _) = file.load_contents_async_future().await?;
    Ok(base64::encode(contents))
}
