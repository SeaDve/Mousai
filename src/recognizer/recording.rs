use serde::{Deserialize, Serialize};

use std::cell::{Cell, Ref, RefCell};

use crate::{core::DateTime, model::Song};

#[derive(Debug, Serialize, Deserialize)]
pub enum RecognizeResult {
    Ok(Song),
    Err {
        /// Whether the failure is permanent (i.e. "no matches found for
        /// this recording", in contrast to "internet connection error" or
        /// "expired token error")
        is_permanent: bool,
    },
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Recording {
    bytes: Vec<u8>,
    recorded_time: DateTime,
    recognize_retries: Cell<u8>,
    recognize_result: RefCell<Option<RecognizeResult>>,
}

impl Recording {
    pub fn new(bytes: Vec<u8>, recorded_time: DateTime) -> Self {
        Self {
            bytes,
            recorded_time,
            recognize_retries: Cell::default(),
            recognize_result: RefCell::default(),
        }
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub fn recorded_time(&self) -> &DateTime {
        &self.recorded_time
    }

    pub fn recognize_retries(&self) -> u8 {
        self.recognize_retries.get()
    }

    pub fn increment_recognize_retries(&self) {
        self.recognize_retries.set(self.recognize_retries.get() + 1);
    }

    pub fn recognize_result(&self) -> Ref<'_, Option<RecognizeResult>> {
        self.recognize_result.borrow()
    }

    pub fn set_recognize_result(&self, result: RecognizeResult) {
        self.recognize_result.replace(Some(result));
    }
}
