use gtk::glib;

#[derive(Debug)]
pub struct AudioRecording {
    bytes: glib::Bytes,
}

impl AudioRecording {
    pub async fn to_base_64(&self) -> Result<String, glib::Error> {
        Ok(glib::base64_encode(&self.bytes).into())
    }
}

impl From<glib::Bytes> for AudioRecording {
    fn from(bytes: glib::Bytes) -> Self {
        Self { bytes }
    }
}
