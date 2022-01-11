use gtk::{
    gio::{self, prelude::*},
    glib,
};

use std::path::{Path, PathBuf};

#[derive(Debug)]
pub struct AudioRecording {
    file: gio::File,
}

impl AudioRecording {
    pub fn new(path: impl AsRef<Path>) -> Self {
        Self {
            file: gio::File::for_path(path.as_ref()),
        }
    }

    pub fn path(&self) -> PathBuf {
        self.file.path().unwrap()
    }

    pub async fn delete(&self) -> anyhow::Result<()> {
        self.file
            .delete_async_future(glib::PRIORITY_DEFAULT_IDLE)
            .await?;

        Ok(())
    }
}
