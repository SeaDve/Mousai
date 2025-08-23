mod mock;
mod response;

use std::time::Duration;

use async_trait::async_trait;
use gtk::{gio, glib};
use serde_json::json;
use soup::prelude::*;

pub use self::mock::AudDMock;
use self::response::Response;
use super::{Provider, RecognizeError, RecognizeErrorKind};
use crate::{Application, external_links::ExternalLinkKey, song::Song, uid::Uid};

#[derive(Debug)]
pub struct AudD {
    api_token: String,
}

impl AudD {
    pub fn new(api_token: Option<&str>) -> Self {
        Self {
            api_token: api_token.unwrap_or_default().to_string(),
        }
    }

    fn build_song_from_response_bytes(response_bytes: &[u8]) -> Result<Song, RecognizeError> {
        let data = serde_json::from_slice::<Response>(response_bytes)
            .map_err(|err| {
                RecognizeError::new(RecognizeErrorKind::OtherPermanent, err.to_string())
            })?
            .data()?;

        let mut song_builder = Song::builder(
            &Uid::from_prefixed("AudD", data.info_link.trim_start_matches("https://lis.tn/")), // Info link is unique to every song
            &data.title,
            &data.artist,
            &data.album,
        );

        if let Some(ref release_date) = data.release_date {
            song_builder.release_date(release_date);
        }

        let mut playback_links = Vec::new();
        let mut album_images = Vec::new();

        song_builder.external_link(ExternalLinkKey::AudDUrl, data.info_link);

        song_builder.external_link(
            ExternalLinkKey::YoutubeSearchTerm,
            format!("{} - {}", data.artist, data.title),
        );

        if let Some(spotify_data) = data.spotify_data {
            if let Some(image) = spotify_data.album.images.first() {
                album_images.push(image.url.clone());
            }

            if !spotify_data.preview_url.is_empty() {
                playback_links.push(spotify_data.preview_url);
            }

            song_builder.external_link(
                ExternalLinkKey::SpotifyUrl,
                spotify_data.external_urls.spotify,
            );
        }

        if let Some(mut apple_music_data) = data.apple_music_data {
            song_builder.external_link(ExternalLinkKey::AppleMusicUrl, &apple_music_data.url);

            if let Some(playback_preview) = apple_music_data.previews.pop() {
                playback_links.push(playback_preview.url);
            }

            album_images.push(
                apple_music_data
                    .artwork
                    .url
                    .replace("{w}", "600")
                    .replace("{h}", "600"),
            );
        }

        if let Some(lyrics_data) = data.lyrics_data
            && !lyrics_data.lyrics.is_empty()
        {
            song_builder.lyrics(&lyrics_data.lyrics);
        }

        if let Some(album_image) = album_images.first() {
            song_builder.album_art_link(album_image);
        }

        if let Some(playback_link) = playback_links.first() {
            song_builder.playback_link(playback_link);
        }

        Ok(song_builder.build())
    }
}

#[async_trait(?Send)]
impl Provider for AudD {
    async fn recognize(&self, bytes: &[u8]) -> Result<Song, RecognizeError> {
        let data = json!({
            "api_token": self.api_token,
            "return": "spotify,apple_music,musicbrainz,lyrics",
            "audio": glib::base64_encode(bytes).as_str(),
        });

        let message = soup::Message::new("POST", "https://api.audd.io/").map_err(|err| {
            RecognizeError::new(
                RecognizeErrorKind::OtherPermanent,
                format!("Failed to create POST message: {}", err),
            )
        })?;
        message.set_request_body_from_bytes(None, Some(&glib::Bytes::from_owned(data.to_string())));
        message.set_priority(soup::MessagePriority::High);

        let response_bytes = Application::get()
            .session()
            .send_and_read_future(&message, glib::Priority::default())
            .await
            .map_err(|err| {
                if err.matches(gio::ResolverError::NotFound)
                    || err.matches(gio::ResolverError::TemporaryFailure)
                {
                    RecognizeError::new(RecognizeErrorKind::Connection, err.to_string())
                } else {
                    RecognizeError::new(RecognizeErrorKind::OtherPermanent, err.to_string())
                }
            })?;

        tracing::trace!(server_response = ?std::str::from_utf8(&response_bytes));

        Self::build_song_from_response_bytes(&response_bytes)
    }

    fn listen_duration(&self) -> Duration {
        Duration::from_secs(5)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    fn parse_response_str(response_str: &'static str) -> Result<Song, RecognizeError> {
        AudD::build_song_from_response_bytes(response_str.as_bytes())
    }

    #[test]
    fn invalid_json() {
        let res = parse_response_str("");
        assert_eq!(res.unwrap_err().kind(), RecognizeErrorKind::OtherPermanent);
    }

    #[test]
    fn no_matches() {
        let res = parse_response_str("{\"status\":\"success\",\"result\":null}");
        assert_eq!(res.unwrap_err().kind(), RecognizeErrorKind::NoMatches);
    }

    #[test]
    fn daily_limit_reached() {
        let res = parse_response_str(
            "{\"status\":\"error\",\"error\":{\"error_code\":901,\"error_message\":\"Recognition failed: authorization failed: no api_token passed and the limit was reached. Get an api_token from dashboard.audd.io.\"},\"request_params\":{},\"request_api_method\":\"recognize\",\"request_http_method\":\"POST\",\"see api documentation\":\"https://docs.audd.io\",\"contact us\":\"api@audd.io\"}",
        );
        assert_eq!(
            res.unwrap_err().kind(),
            RecognizeErrorKind::TokenLimitReached,
        );
    }

    #[test]
    fn wrong_api_token() {
        let res = parse_response_str(
            "{\"status\":\"error\",\"error\":{\"error_code\":900,\"error_message\":\"Recognition failed: authorization failed: wrong api_token. Please check if your account is activated on dashboard.audd.io and has either a trial or an active subscription.\"},\"request_params\":{},\"request_api_method\":\"recognize\",\"request_http_method\":\"POST\",\"see api documentation\":\"https://docs.audd.io\",\"contact us\":\"api@audd.io\"}",
        );
        assert_eq!(res.unwrap_err().kind(), RecognizeErrorKind::InvalidToken);
    }

    #[test]
    fn wrong_file_sent_or_audio_without_streams() {
        let res = parse_response_str(
            "{\"status\":\"error\",\"error\":{\"error_code\":300,\"error_message\":\"Recognition failed: a problem with fingerprints creating. Keep in mind that you should send only audio files or links to audio files. We support some of the Instagram, Twitter, TikTok and Facebook videos, and also parse html for OpenGraph and JSON-LD media and \\u003caudio\\u003e/\\u003cvideo\\u003e tags, but it's always better to send a 10-20 seconds-long audio file. For audio streams, see https://docs.audd.io/streams/\"},\"request_params\":{},\"request_api_method\":\"recognize\",\"request_http_method\":\"POST\",\"see api documentation\":\"https://docs.audd.io\",\"contact us\":\"api@audd.io\"}",
        );
        assert_eq!(res.unwrap_err().kind(), RecognizeErrorKind::Fingerprint);
    }

    #[test]
    fn proper_but_no_spotify_field() {
        // TODO add more test, when we added functionality that uses spotify data
        let res = parse_response_str(
            "{\"status\":\"success\",\"result\":{\"artist\":\"The London Symphony Orchestra\",\"title\":\"Eine Kleine Nachtmusik\",\"album\":\"An Hour Of The London Symphony Orchestra\",\"release_date\":\"2014-04-22\",\"label\":\"Glory Days Music\",\"timecode\":\"00:24\",\"song_link\":\"https://lis.tn/EineKleineNachtmusik\"}}",
        );
        let song = res.unwrap();
        assert_eq!(song.title(), "Eine Kleine Nachtmusik");
        assert_eq!(song.artist(), "The London Symphony Orchestra");
        assert_eq!(song.album(), "An Hour Of The London Symphony Orchestra");
        assert_eq!(song.release_date().as_deref(), Some("2014-04-22"));
        assert_eq!(
            song.external_links()
                .get(ExternalLinkKey::AudDUrl)
                .as_deref(),
            Some("https://lis.tn/EineKleineNachtmusik")
        );
        assert_eq!(song.external_links().get(ExternalLinkKey::SpotifyUrl), None);
        assert_eq!(song.album_art_link(), None);
        assert_eq!(song.playback_link(), None);

        let res = parse_response_str(
            "{\"status\":\"success\",\"result\":{\"artist\":\"Public\",\"title\":\"Make You Mine\",\"album\":\"Let's Make It\",\"release_date\":\"2014-10-07\",\"label\":\"PUBLIC\",\"timecode\":\"00:43\",\"song_link\":\"https://lis.tn/FUYgUV\"}}",
        );
        let song = res.unwrap();
        assert_eq!(song.title(), "Make You Mine");
        assert_eq!(song.artist(), "Public");
        assert_eq!(song.album(), "Let's Make It");
        assert_eq!(song.release_date().as_deref(), Some("2014-10-07"));
        assert_eq!(
            song.external_links()
                .get(ExternalLinkKey::AudDUrl)
                .as_deref(),
            Some("https://lis.tn/FUYgUV")
        );
        assert_eq!(song.external_links().get(ExternalLinkKey::SpotifyUrl), None);
        assert_eq!(song.album_art_link(), None);
        assert_eq!(song.playback_link(), None);
    }

    #[test]
    fn proper() {
        let res = parse_response_str(
            "{\"status\":\"success\",\"result\":{\"artist\":\"5 Seconds Of Summer\",\"title\":\"Amnesia\",\"album\":\"Amnesia\",\"release_date\":\"2014-06-24\",\"label\":\"Universal Music\",\"timecode\":\"01:02\",\"song_link\":\"https://lis.tn/WSKAzD\",\"spotify\":{\"album\":{\"name\":\"5 Seconds Of Summer\",\"artists\":[{\"name\":\"5 Seconds of Summer\",\"id\":\"5Rl15oVamLq7FbSb0NNBNy\",\"uri\":\"spotify:artist:5Rl15oVamLq7FbSb0NNBNy\",\"href\":\"https://api.spotify.com/v1/artists/5Rl15oVamLq7FbSb0NNBNy\",\"external_urls\":{\"spotify\":\"https://open.spotify.com/artist/5Rl15oVamLq7FbSb0NNBNy\"}}],\"album_group\":\"\",\"album_type\":\"album\",\"id\":\"2LkWHNNHgD6BRNeZI2SL1L\",\"uri\":\"spotify:album:2LkWHNNHgD6BRNeZI2SL1L\",\"available_markets\":null,\"href\":\"https://api.spotify.com/v1/albums/2LkWHNNHgD6BRNeZI2SL1L\",\"images\":[{\"height\":640,\"width\":640,\"url\":\"https://i.scdn.co/image/ab67616d0000b27393432e914046a003229378da\"},{\"height\":300,\"width\":300,\"url\":\"https://i.scdn.co/image/ab67616d00001e0293432e914046a003229378da\"},{\"height\":64,\"width\":64,\"url\":\"https://i.scdn.co/image/ab67616d0000485193432e914046a003229378da\"}],\"external_urls\":{\"spotify\":\"https://open.spotify.com/album/2LkWHNNHgD6BRNeZI2SL1L\"},\"release_date\":\"2014-06-27\",\"release_date_precision\":\"day\"},\"external_ids\":{\"isrc\":\"GBUM71401926\"},\"popularity\":69,\"is_playable\":true,\"linked_from\":null,\"artists\":[{\"name\":\"5 Seconds of Summer\",\"id\":\"5Rl15oVamLq7FbSb0NNBNy\",\"uri\":\"spotify:artist:5Rl15oVamLq7FbSb0NNBNy\",\"href\":\"https://api.spotify.com/v1/artists/5Rl15oVamLq7FbSb0NNBNy\",\"external_urls\":{\"spotify\":\"https://open.spotify.com/artist/5Rl15oVamLq7FbSb0NNBNy\"}}],\"available_markets\":null,\"disc_number\":1,\"duration_ms\":237247,\"explicit\":false,\"external_urls\":{\"spotify\":\"https://open.spotify.com/track/1JCCdiru7fhstOIF4N7WJC\"},\"href\":\"https://api.spotify.com/v1/tracks/1JCCdiru7fhstOIF4N7WJC\",\"id\":\"1JCCdiru7fhstOIF4N7WJC\",\"name\":\"Amnesia\",\"preview_url\":\"\",\"track_number\":12,\"uri\":\"spotify:track:1JCCdiru7fhstOIF4N7WJC\"}}}",
        );
        let song = res.unwrap();
        assert_eq!(song.title(), "Amnesia");
        assert_eq!(song.artist(), "5 Seconds Of Summer");
        assert_eq!(song.album(), "Amnesia");
        assert_eq!(song.release_date().as_deref(), Some("2014-06-24"));
        assert_eq!(
            song.external_links()
                .get(ExternalLinkKey::AudDUrl)
                .as_deref(),
            Some("https://lis.tn/WSKAzD")
        );
        assert_eq!(
            song.external_links()
                .get(ExternalLinkKey::SpotifyUrl)
                .as_deref(),
            Some("https://open.spotify.com/track/1JCCdiru7fhstOIF4N7WJC")
        );
        assert_eq!(
            song.album_art_link().as_deref(),
            Some("https://i.scdn.co/image/ab67616d0000b27393432e914046a003229378da")
        );
        assert_eq!(song.playback_link(), None);

        let res = parse_response_str(
            "{\"status\":\"success\",\"result\":{\"artist\":\"Alessia Cara\",\"title\":\"Scars To Your Beautiful\",\"album\":\"Know-It-All\",\"release_date\":\"2015-11-13\",\"label\":\"EP Entertainment, LLC / Def Jam\",\"timecode\":\"00:28\",\"song_link\":\"https://lis.tn/ScarsToYourBeautiful\",\"spotify\":{\"album\":{\"name\":\"Know-It-All (Deluxe)\",\"artists\":[{\"name\":\"Alessia Cara\",\"id\":\"2wUjUUtkb5lvLKcGKsKqsR\",\"uri\":\"spotify:artist:2wUjUUtkb5lvLKcGKsKqsR\",\"href\":\"https://api.spotify.com/v1/artists/2wUjUUtkb5lvLKcGKsKqsR\",\"external_urls\":{\"spotify\":\"https://open.spotify.com/artist/2wUjUUtkb5lvLKcGKsKqsR\"}}],\"album_group\":\"\",\"album_type\":\"album\",\"id\":\"3rDbA12I5duZnlwakqDdZa\",\"uri\":\"spotify:album:3rDbA12I5duZnlwakqDdZa\",\"available_markets\":null,\"href\":\"https://api.spotify.com/v1/albums/3rDbA12I5duZnlwakqDdZa\",\"images\":[{\"height\":640,\"width\":640,\"url\":\"https://i.scdn.co/image/ab67616d0000b273e3ae597159d6c2541c4ee61b\"},{\"height\":300,\"width\":300,\"url\":\"https://i.scdn.co/image/ab67616d00001e02e3ae597159d6c2541c4ee61b\"},{\"height\":64,\"width\":64,\"url\":\"https://i.scdn.co/image/ab67616d00004851e3ae597159d6c2541c4ee61b\"}],\"external_urls\":{\"spotify\":\"https://open.spotify.com/album/3rDbA12I5duZnlwakqDdZa\"},\"release_date\":\"2015-11-13\",\"release_date_precision\":\"day\"},\"external_ids\":{\"isrc\":\"USUM71506811\"},\"popularity\":75,\"is_playable\":true,\"linked_from\":null,\"artists\":[{\"name\":\"Alessia Cara\",\"id\":\"2wUjUUtkb5lvLKcGKsKqsR\",\"uri\":\"spotify:artist:2wUjUUtkb5lvLKcGKsKqsR\",\"href\":\"https://api.spotify.com/v1/artists/2wUjUUtkb5lvLKcGKsKqsR\",\"external_urls\":{\"spotify\":\"https://open.spotify.com/artist/2wUjUUtkb5lvLKcGKsKqsR\"}}],\"available_markets\":null,\"disc_number\":1,\"duration_ms\":230226,\"explicit\":false,\"external_urls\":{\"spotify\":\"https://open.spotify.com/track/0prNGof3XqfTvNDxHonvdK\"},\"href\":\"https://api.spotify.com/v1/tracks/0prNGof3XqfTvNDxHonvdK\",\"id\":\"0prNGof3XqfTvNDxHonvdK\",\"name\":\"Scars To Your Beautiful\",\"preview_url\":\"\",\"track_number\":10,\"uri\":\"spotify:track:0prNGof3XqfTvNDxHonvdK\"}}}",
        );
        let song = res.unwrap();
        assert_eq!(song.title(), "Scars To Your Beautiful");
        assert_eq!(song.artist(), "Alessia Cara");
        assert_eq!(song.album(), "Know-It-All");
        assert_eq!(song.release_date().as_deref(), Some("2015-11-13"));
        assert_eq!(
            song.external_links()
                .get(ExternalLinkKey::AudDUrl)
                .as_deref(),
            Some("https://lis.tn/ScarsToYourBeautiful")
        );
        assert_eq!(
            song.external_links()
                .get(ExternalLinkKey::SpotifyUrl)
                .as_deref(),
            Some("https://open.spotify.com/track/0prNGof3XqfTvNDxHonvdK")
        );
        assert_eq!(
            song.album_art_link().as_deref(),
            Some("https://i.scdn.co/image/ab67616d0000b273e3ae597159d6c2541c4ee61b")
        );
        assert_eq!(song.playback_link(), None);

        let res = parse_response_str(
            "{\"status\":\"success\",\"result\":{\"artist\":\"Daniel Boone\",\"title\":\"Beautiful Sunday\",\"album\":\"Pop Legend Vol.1\",\"release_date\":\"2010-01-15\",\"label\":\"Open Records\",\"timecode\":\"00:33\",\"song_link\":\"https://lis.tn/YTuccJ\",\"spotify\":{\"album\":{\"name\":\"Cocktail Super Pop\",\"artists\":[{\"name\":\"Various Artists\",\"id\":\"0LyfQWJT6nXafLPZqxe9Of\",\"uri\":\"spotify:artist:0LyfQWJT6nXafLPZqxe9Of\",\"href\":\"https://api.spotify.com/v1/artists/0LyfQWJT6nXafLPZqxe9Of\",\"external_urls\":{\"spotify\":\"https://open.spotify.com/artist/0LyfQWJT6nXafLPZqxe9Of\"}}],\"album_group\":\"\",\"album_type\":\"compilation\",\"id\":\"1ZsLymIsvlHEnGtQFen5xd\",\"uri\":\"spotify:album:1ZsLymIsvlHEnGtQFen5xd\",\"available_markets\":null,\"href\":\"https://api.spotify.com/v1/albums/1ZsLymIsvlHEnGtQFen5xd\",\"images\":[{\"height\":640,\"width\":640,\"url\":\"https://i.scdn.co/image/ab67616d0000b273db8f64a52a4ec4cde9a9528a\"},{\"height\":300,\"width\":300,\"url\":\"https://i.scdn.co/image/ab67616d00001e02db8f64a52a4ec4cde9a9528a\"},{\"height\":64,\"width\":64,\"url\":\"https://i.scdn.co/image/ab67616d00004851db8f64a52a4ec4cde9a9528a\"}],\"external_urls\":{\"spotify\":\"https://open.spotify.com/album/1ZsLymIsvlHEnGtQFen5xd\"},\"release_date\":\"2013-01-18\",\"release_date_precision\":\"day\"},\"external_ids\":{\"isrc\":\"ES5530914999\"},\"popularity\":0,\"is_playable\":true,\"linked_from\":null,\"artists\":[{\"name\":\"Daniel Boone\",\"id\":\"3M5aUsJmembbwKbUx434lS\",\"uri\":\"spotify:artist:3M5aUsJmembbwKbUx434lS\",\"href\":\"https://api.spotify.com/v1/artists/3M5aUsJmembbwKbUx434lS\",\"external_urls\":{\"spotify\":\"https://open.spotify.com/artist/3M5aUsJmembbwKbUx434lS\"}}],\"available_markets\":null,\"disc_number\":1,\"duration_ms\":176520,\"explicit\":false,\"external_urls\":{\"spotify\":\"https://open.spotify.com/track/6o3AMOtlfI6APSUooekMtt\"},\"href\":\"https://api.spotify.com/v1/tracks/6o3AMOtlfI6APSUooekMtt\",\"id\":\"6o3AMOtlfI6APSUooekMtt\",\"name\":\"Beautiful Sunday\",\"preview_url\":\"https://p.scdn.co/mp3-preview/b2fa24732fe08a251b0c8d44774f37fd55378378?cid=e44e7b8278114c7db211c00ea273ac69\",\"track_number\":16,\"uri\":\"spotify:track:6o3AMOtlfI6APSUooekMtt\"}}}",
        );
        let song = res.unwrap();
        assert_eq!(song.title(), "Beautiful Sunday");
        assert_eq!(song.artist(), "Daniel Boone");
        assert_eq!(song.album(), "Pop Legend Vol.1");
        assert_eq!(song.release_date().as_deref(), Some("2010-01-15"));
        assert_eq!(
            song.external_links()
                .get(ExternalLinkKey::AudDUrl)
                .as_deref(),
            Some("https://lis.tn/YTuccJ")
        );
        assert_eq!(
            song.external_links()
                .get(ExternalLinkKey::SpotifyUrl)
                .as_deref(),
            Some("https://open.spotify.com/track/6o3AMOtlfI6APSUooekMtt")
        );
        assert_eq!(
            song.album_art_link().as_deref(),
            Some("https://i.scdn.co/image/ab67616d0000b273db8f64a52a4ec4cde9a9528a")
        );
        assert_eq!(
            song.playback_link().as_deref(),
            Some(
                "https://p.scdn.co/mp3-preview/b2fa24732fe08a251b0c8d44774f37fd55378378?cid=e44e7b8278114c7db211c00ea273ac69"
            )
        );
    }
}
