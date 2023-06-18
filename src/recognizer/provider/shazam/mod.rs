// This module is based on code from SongRec (GPLv3)
// See https://github.com/marin-m/SongRec/tree/10bb94bec68df7f49784573b6b97b46ff98c6a5a/src/fingerprinting

mod hanning;
mod signature;
mod signature_generator;
mod user_agents;

use async_trait::async_trait;
use gtk::{gio, glib};
use rodio::source::UniformSourceIterator;
use serde_json::json;
use soup::prelude::*;

use std::{io::Cursor, time::Duration, time::SystemTime};

use self::{
    signature::DecodedSignature, signature_generator::SignatureGenerator, user_agents::USER_AGENTS,
};
use super::{Provider, RecognizeError, RecognizeErrorKind, Song};
use crate::{
    model::{ExternalLinkKey, Uid},
    utils,
};

const SAMPLE_RATE_HZ: usize = 16_000;
const MAX_AUDIO_DURATION_SECS: usize = 12;

#[derive(Debug)]
pub struct Shazam;

#[async_trait(?Send)]
impl Provider for Shazam {
    async fn recognize(&self, bytes: &glib::Bytes) -> Result<Song, RecognizeError> {
        let bytes = bytes.clone();
        let signature = gio::spawn_blocking(move || create_signature(bytes))
            .await
            .map_err(|err| {
                RecognizeError::with_message(
                    RecognizeErrorKind::OtherPermanent,
                    format!("Failed to spawn task: {:?}", err),
                )
            })??;
        let response_bytes = send_request(signature).await?;

        tracing::trace!(server_response = ?std::str::from_utf8(&response_bytes));

        // gio::File::for_path(glib::home_dir().join("test.json"))
        //     .replace_contents(
        //         &response,
        //         None,
        //         false,
        //         gio::FileCreateFlags::NONE,
        //         gio::Cancellable::NONE,
        //     )
        //     .unwrap();

        build_song_from_response_bytes(&response_bytes)
    }

    fn listen_duration(&self) -> Duration {
        Duration::from_secs(4)
    }
}

fn create_signature(bytes: glib::Bytes) -> Result<DecodedSignature, RecognizeError> {
    let decoder = rodio::Decoder::new(Cursor::new(bytes)).map_err(|err| {
        RecognizeError::with_message(
            RecognizeErrorKind::Fingerprint,
            format!("Failed to decode bytes: {}", err),
        )
    })?;

    // Downsample the raw PCM samples to 16 KHz
    let raw_pcm_samples =
        UniformSourceIterator::new(decoder, 1, SAMPLE_RATE_HZ as u32).collect::<Vec<i16>>();

    // Skip to the middle to increase recognition odds and take a maximum of 12 seconds of audio.
    let raw_pcm_samples_slice = {
        let middle = raw_pcm_samples.len() / 2;
        let offset = raw_pcm_samples
            .len()
            .clamp(0, MAX_AUDIO_DURATION_SECS * SAMPLE_RATE_HZ)
            / 2;
        &raw_pcm_samples[middle - offset..middle + offset]
    };

    Ok(SignatureGenerator::make_signature_from_buffer(
        raw_pcm_samples_slice,
    ))
}

async fn send_request(signature: DecodedSignature) -> Result<glib::Bytes, RecognizeError> {
    let signature_uri = signature.encode_to_uri().map_err(|err| {
        RecognizeError::with_message(
            RecognizeErrorKind::Fingerprint,
            format!("Failed to encode signature to URI: {}", err),
        )
    })?;
    let timestamp_ms = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .expect("system time must be after unix epoch")
        .as_millis();
    let post_data = json!({
        "geolocation": {
            "altitude": 300,
            "latitude": 45,
            "longitude": 2
        },
        "signature": {
            "samplems": (signature.number_samples as f32 / signature.sample_rate_hz as f32 * 1000.) as u32,
            "timestamp": timestamp_ms as u32,
            "uri": signature_uri
        },
        "timestamp": timestamp_ms as u32,
        "timezone": "Europe/Paris"
    }).to_string();

    let uri = format!(
        "https://amp.shazam.com/discovery/v5/en/US/android/-/tag/{}/{}?sync=true&webv3=true&sampling=true&connected=&shazamapiversion=v3&sharehub=true&video=v3",
        glib::uuid_string_random(),
        glib::uuid_string_random()
    );

    tracing::trace!(uri, post_data);

    let message = soup::Message::new("POST", &uri).map_err(|err| {
        RecognizeError::with_message(
            RecognizeErrorKind::OtherPermanent,
            format!("Failed to create POST message: {}", err),
        )
    })?;
    message.set_priority(soup::MessagePriority::High);
    message.set_request_body_from_bytes(
        Some("application/json"),
        Some(&glib::Bytes::from_owned(post_data)),
    );

    let headers = message
        .request_headers()
        .expect("message must have request headers");
    headers.append(
        "User-Agent",
        USER_AGENTS
            .get(glib::random_int_range(0, USER_AGENTS.len() as i32) as usize)
            .expect("range must be valid"),
    );
    headers.append("Content-Language", "en_US");

    let response = utils::app_instance()
        .session()
        .send_and_read_future(&message, glib::PRIORITY_DEFAULT)
        .await
        .map_err(|err| {
            if err.matches(gio::ResolverError::NotFound)
                || err.matches(gio::ResolverError::TemporaryFailure)
            {
                RecognizeError::with_message(RecognizeErrorKind::Connection, err.to_string())
            } else {
                RecognizeError::with_message(RecognizeErrorKind::OtherPermanent, err.to_string())
            }
        })?;

    Ok(response)
}

fn build_song_from_response_bytes(response_bytes: &[u8]) -> Result<Song, RecognizeError> {
    let value = serde_json::from_slice::<serde_json::Value>(response_bytes).map_err(|err| {
        RecognizeError::with_message(RecognizeErrorKind::OtherPermanent, err.to_string())
    })?;

    match value["matches"].as_array() {
        Some(matches) => match matches.len() {
            0 => return Err(RecognizeError::new(RecognizeErrorKind::NoMatches)),
            1 => {}
            _ => {
                tracing::debug!(?matches, "Multiple matches found");
            }
        },
        None => {
            return Err(RecognizeError::with_message(
                RecognizeErrorKind::OtherPermanent,
                "missing `matches` field",
            ))
        }
    }

    let mut song_section = None;
    let mut lyrics_section = None;
    if let Some(sections) = value["track"]["sections"].as_array() {
        for section in sections {
            match section["type"].as_str() {
                Some("SONG") if song_section.is_none() => {
                    song_section = Some(section);
                }
                Some("LYRICS") if lyrics_section.is_none() => {
                    lyrics_section = Some(section);
                }
                Some(_) | None => continue,
            }
        }
    }

    let mut album = None;
    let mut release_date = None;
    if let Some(song_section) = song_section {
        if let Some(metadata) = song_section["metadata"].as_array() {
            for item in metadata {
                match item["title"].as_str() {
                    Some("Album") if album.is_none() => {
                        if let Some(value) = item["text"].as_str() {
                            album = Some(value);
                        }
                    }
                    Some("Released") if release_date.is_none() => {
                        if let Some(value) = item["text"].as_str() {
                            release_date = Some(value);
                        }
                    }
                    Some(_) | None => continue,
                }
            }
        }
    }

    let title = value_as_str_or_err(&value["track"]["title"])?;
    let artist = value_as_str_or_err(&value["track"]["subtitle"])?;
    let mut song_builder = Song::builder(
        &Uid::from_prefixed("Shazam", value_as_str_or_err(&value["track"]["key"])?),
        title,
        artist,
        album.unwrap_or_default(),
    );

    song_builder.external_link(
        ExternalLinkKey::YoutubeSearchTerm,
        format!("{} - {}", artist, title),
    );

    if let Some(coverart) = value["track"]["images"]["coverart"].as_str() {
        song_builder.album_art_link(coverart);
    }

    if let Some(release_date) = release_date {
        song_builder.release_date(release_date);
    }

    if let Some(lyrics_section) = lyrics_section {
        if let Some(lyrics) = lyrics_section["text"].as_array() {
            song_builder.lyrics(
                &lyrics
                    .iter()
                    .filter_map(|v| v.as_str())
                    .collect::<Vec<_>>()
                    .join("\n"),
            );
        }
    }

    if let Some(actions) = value["track"]["hub"]["actions"].as_array() {
        for action in actions {
            if action["type"] == "uri" {
                if let Some(playback_link) = action["uri"].as_str() {
                    song_builder.playback_link(playback_link);
                }
                break;
            }
        }
    }

    if let Some(providers) = value["track"]["hub"]["providers"].as_array() {
        for provider in providers {
            match provider["type"].as_str() {
                Some("SPOTIFY") => {
                    if let Some(actions) = provider["actions"].as_array() {
                        for action in actions {
                            if action["type"] == "uri" {
                                if let Some(uri) = action["uri"].as_str() {
                                    song_builder.external_link(ExternalLinkKey::SpotifyUrl, uri);
                                }
                            }
                        }
                    }
                }
                Some(_) | None => continue,
            }
        }
    }

    Ok(song_builder.build())
}

fn value_as_str_or_err(value: &serde_json::Value) -> Result<&str, RecognizeError> {
    value.as_str().ok_or_else(|| {
        RecognizeError::with_message(
            RecognizeErrorKind::OtherPermanent,
            format!("expected `str` but got `{}`", value),
        )
    })
}
