use serde::{Deserialize, Serialize};

use std::cell::{Cell, Ref, RefCell};

use super::RecognizeError;
use crate::{core::DateTime, model::Song};

#[derive(Debug, Serialize, Deserialize)]
pub struct Recording {
    bytes: Vec<u8>,
    recorded_time: DateTime,
    recognize_result: RefCell<Option<Result<Song, RecognizeError>>>,

    #[serde(skip)] // So we can retry next session
    recognize_retries: Cell<u8>,
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

    pub fn recognize_result(&self) -> Ref<'_, Option<Result<Song, RecognizeError>>> {
        self.recognize_result.borrow()
    }

    pub fn set_recognize_result(&self, result: Result<Song, RecognizeError>) {
        self.recognize_result.replace(Some(result));
    }

    pub fn recognize_retries(&self) -> u8 {
        self.recognize_retries.get()
    }

    pub fn increment_recognize_retries(&self) {
        self.recognize_retries.set(self.recognize_retries.get() + 1);
    }
}
