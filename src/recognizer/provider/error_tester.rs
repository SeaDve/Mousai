use async_trait::async_trait;
use gtk::glib;

use std::sync::atomic::{AtomicI32, Ordering};

use super::{RecognizeError, RecognizeErrorKind, Song, TestProvider, TestProviderMode};

// FIXME Store this state to the struct
static CURRENT: AtomicI32 = AtomicI32::new(0);

#[derive(Debug)]
pub struct ErrorTester;

#[async_trait(?Send)]
impl TestProvider for ErrorTester {
    async fn recognize_impl(
        &self,
        _: &glib::Bytes,
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
            0 => RecognizeError::with_message(
                RecognizeErrorKind::Connection,
                "connection error message",
            ),
            1 => RecognizeError::with_message(
                RecognizeErrorKind::TokenLimitReached,
                "token limit reached error message",
            ),
            2 => RecognizeError::with_message(
                RecognizeErrorKind::InvalidToken,
                "invalid token error message",
            ),
            3 => RecognizeError::with_message(
                RecognizeErrorKind::Fingerprint,
                "fingerprint error message",
            ),
            4 => RecognizeError::with_message(
                RecognizeErrorKind::NoMatches,
                "no matches error message",
            ),
            5 => RecognizeError::with_message(
                RecognizeErrorKind::OtherPermanent,
                "other permanent error message",
            ),
            _ => unreachable!("current must always be less than n variants of RecognizeErrorKind"),
        };

        Err(err)
    }
}
