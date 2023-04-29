use async_trait::async_trait;

use std::sync::atomic::{AtomicI32, Ordering};

use super::Song;
use crate::recognizer::provider::{
    RecognizeError, RecognizeErrorKind, TestProvider, TestProviderMode,
};

// FIXME Store this state to the struct
static CURRENT: AtomicI32 = AtomicI32::new(0);

#[derive(Debug)]
pub struct ErrorTester;

#[async_trait(?Send)]
impl TestProvider for ErrorTester {
    async fn recognize_impl(
        &self,
        _: &[u8],
        mode: TestProviderMode,
    ) -> Result<Song, RecognizeError> {
        if mode != TestProviderMode::ErrorOnly {
            tracing::warn!("ErrorTester can only handle ErrorOnly mode");
        }

        let err = match CURRENT
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |prev_value| {
                if prev_value >= 5 {
                    Some(0)
                } else {
                    Some(prev_value + 1)
                }
            })
            .unwrap()
        {
            0 => RecognizeError::new(
                RecognizeErrorKind::Connection,
                "connection error message".to_string(),
            ),
            1 => RecognizeError::new(
                RecognizeErrorKind::TokenLimitReached,
                "token limit reached error message".to_string(),
            ),
            2 => RecognizeError::new(
                RecognizeErrorKind::InvalidToken,
                "invalid token error message".to_string(),
            ),
            3 => RecognizeError::new(
                RecognizeErrorKind::Fingerprint,
                "fingerprint error message".to_string(),
            ),
            4 => RecognizeError::new(
                RecognizeErrorKind::NoMatches,
                "no matches error message".to_string(),
            ),
            5 => RecognizeError::new(
                RecognizeErrorKind::OtherPermanent,
                "other permanent error message".to_string(),
            ),
            _ => unreachable!("current must always be less than n variants of RecognizeErrorKind"),
        };

        Err(err)
    }
}
