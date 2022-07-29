use gtk::glib;

#[derive(Debug)]
pub struct AudioRecording {
    bytes: glib::Bytes,
}

impl AudioRecording {
    pub fn to_base_64(&self) -> Result<glib::GString, glib::Error> {
        Ok(glib::base64_encode(&self.bytes))
    }
}

impl From<glib::Bytes> for AudioRecording {
    fn from(bytes: glib::Bytes) -> Self {
        Self { bytes }
    }
}
