mod error;
mod response;

use async_trait::async_trait;
use bytes::Bytes;
use reqwest::Client;
use serde_json::json;

use std::time::Duration;

pub use self::error::Error;
use self::response::{Data, Response};
use super::{Provider, ProviderError};
use crate::{core::AudioRecording, model::Song, utils, RUNTIME};

#[derive(Debug)]
pub struct AudD {
    client: Client,
    api_token: String,
}

impl AudD {
    pub fn new(api_token: Option<&str>) -> Self {
        Self {
            client: Client::new(),
            api_token: api_token.unwrap_or_default().to_string(),
        }
    }

    fn parse(json_bytes: &Bytes) -> Result<Data, Error> {
        let response: Response = serde_json::from_slice(json_bytes)?;
        Ok(response.data()?)
    }

    async fn recognize_inner(&self, recording: &AudioRecording) -> Result<Song, Error> {
        let data = json!({
            "api_token": self.api_token,
            "return": "spotify",
            "audio": utils::file_to_base64(recording.path()).await.map_err(Error::FileConvert)?,
        });

        let server_response = RUNTIME
            .spawn(
                self.client
                    .post("https://api.audd.io/")
                    .body(data.to_string())
                    .send(),
            )
            .await
            .unwrap()?;

        let bytes = RUNTIME.spawn(server_response.bytes()).await.unwrap()?;

        match std::str::from_utf8(&bytes) {
            Ok(string) => log::debug!("server_response: {}", string),
            Err(err) => log::warn!("Failed to get str from `Bytes`: {:?}", err),
        }

        let data = Self::parse(&bytes)?;
        let song = Song::new(&data.title, &data.artist, &data.info_link);

        if let Some(spotify_data) = data.spotify_data {
            if let Some(image) = spotify_data.album.images.get(0) {
                song.set_album_art_link(Some(&image.url));
            }
        }

        Ok(song)
    }
}

#[async_trait(?Send)]
impl Provider for AudD {
    async fn recognize(&self, recording: &AudioRecording) -> Result<Song, ProviderError> {
        self.recognize_inner(recording)
            .await
            .map_err(ProviderError::AudD)
    }

    fn listen_duration(&self) -> Duration {
        Duration::from_secs(5)
    }
}

impl Default for AudD {
    fn default() -> Self {
        Self::new(None)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use error::AudDError;

    fn parse_response(response: &'static str) -> Result<Data, Error> {
        AudD::parse(&Bytes::from_static(response.as_bytes()))
    }

    #[test]
    fn no_matches() {
        let res = parse_response("{\"status\":\"success\",\"result\":null}");
        assert!(matches!(
            res.unwrap_err(),
            Error::AudD(AudDError::NoMatches)
        ));
    }

    #[test]
    fn daily_limit_reached() {
        let res = parse_response("{\"status\":\"error\",\"error\":{\"error_code\":901,\"error_message\":\"Recognition failed: authorization failed: no api_token passed and the limit was reached. Get an api_token from dashboard.audd.io.\"},\"request_params\":{},\"request_api_method\":\"recognize\",\"request_http_method\":\"POST\",\"see api documentation\":\"https://docs.audd.io\",\"contact us\":\"api@audd.io\"}");
        assert!(matches!(
            res.unwrap_err(),
            Error::AudD(AudDError::DailyLimitReached)
        ));
    }

    #[test]
    fn wrong_api_token() {
        let res = parse_response("{\"status\":\"error\",\"error\":{\"error_code\":900,\"error_message\":\"Recognition failed: authorization failed: wrong api_token. Please check if your account is activated on dashboard.audd.io and has either a trial or an active subscription.\"},\"request_params\":{},\"request_api_method\":\"recognize\",\"request_http_method\":\"POST\",\"see api documentation\":\"https://docs.audd.io\",\"contact us\":\"api@audd.io\"}");
        assert!(matches!(
            res.unwrap_err(),
            Error::AudD(AudDError::InvalidToken)
        ));
    }

    #[test]
    fn wrong_file_sent_or_audio_without_streams() {
        let res = parse_response("{\"status\":\"error\",\"error\":{\"error_code\":300,\"error_message\":\"Recognition failed: a problem with fingerprints creating. Keep in mind that you should send only audio files or links to audio files. We support some of the Instagram, Twitter, TikTok and Facebook videos, and also parse html for OpenGraph and JSON-LD media and \\u003caudio\\u003e/\\u003cvideo\\u003e tags, but it's always better to send a 10-20 seconds-long audio file. For audio streams, see https://docs.audd.io/streams/\"},\"request_params\":{},\"request_api_method\":\"recognize\",\"request_http_method\":\"POST\",\"see api documentation\":\"https://docs.audd.io\",\"contact us\":\"api@audd.io\"}");

        match res {
            Err(Error::AudD(AudDError::Fingerprint(ref message))) => assert_eq!(message, "Recognition failed: a problem with fingerprints creating. Keep in mind that you should send only audio files or links to audio files. We support some of the Instagram, Twitter, TikTok and Facebook videos, and also parse html for OpenGraph and JSON-LD media and <audio>/<video> tags, but it's always better to send a 10-20 seconds-long audio file. For audio streams, see https://docs.audd.io/streams/"),
            invalid => panic!("Mismatched result. Got {:?}", invalid),
        }
    }

    #[test]
    fn proper_but_no_spotify_field() {
        // TODO add more test, when we added functionality that uses spotify data
        let res1 = parse_response("{\"status\":\"success\",\"result\":{\"artist\":\"The London Symphony Orchestra\",\"title\":\"Eine Kleine Nachtmusik\",\"album\":\"An Hour Of The London Symphony Orchestra\",\"release_date\":\"2014-04-22\",\"label\":\"Glory Days Music\",\"timecode\":\"00:24\",\"song_link\":\"https://lis.tn/EineKleineNachtmusik\"}}");
        let data1 = res1.unwrap();
        assert!(&data1.spotify_data.is_none());
        assert_eq!(&data1.artist, "The London Symphony Orchestra");
        assert_eq!(&data1.title, "Eine Kleine Nachtmusik");
        assert_eq!(&data1.info_link, "https://lis.tn/EineKleineNachtmusik");

        let res2 = parse_response("{\"status\":\"success\",\"result\":{\"artist\":\"Public\",\"title\":\"Make You Mine\",\"album\":\"Let's Make It\",\"release_date\":\"2014-10-07\",\"label\":\"PUBLIC\",\"timecode\":\"00:43\",\"song_link\":\"https://lis.tn/FUYgUV\"}}");
        let data2 = res2.unwrap();
        assert!(&data1.spotify_data.is_none());
        assert_eq!(&data2.artist, "Public");
        assert_eq!(&data2.title, "Make You Mine");
        assert_eq!(&data2.info_link, "https://lis.tn/FUYgUV");
    }

    #[test]
    fn proper() {
        let res1 = parse_response("{\"status\":\"success\",\"result\":{\"artist\":\"5 Seconds Of Summer\",\"title\":\"Amnesia\",\"album\":\"Amnesia\",\"release_date\":\"2014-06-24\",\"label\":\"Universal Music\",\"timecode\":\"01:02\",\"song_link\":\"https://lis.tn/WSKAzD\",\"spotify\":{\"album\":{\"name\":\"5 Seconds Of Summer\",\"artists\":[{\"name\":\"5 Seconds of Summer\",\"id\":\"5Rl15oVamLq7FbSb0NNBNy\",\"uri\":\"spotify:artist:5Rl15oVamLq7FbSb0NNBNy\",\"href\":\"https://api.spotify.com/v1/artists/5Rl15oVamLq7FbSb0NNBNy\",\"external_urls\":{\"spotify\":\"https://open.spotify.com/artist/5Rl15oVamLq7FbSb0NNBNy\"}}],\"album_group\":\"\",\"album_type\":\"album\",\"id\":\"2LkWHNNHgD6BRNeZI2SL1L\",\"uri\":\"spotify:album:2LkWHNNHgD6BRNeZI2SL1L\",\"available_markets\":null,\"href\":\"https://api.spotify.com/v1/albums/2LkWHNNHgD6BRNeZI2SL1L\",\"images\":[{\"height\":640,\"width\":640,\"url\":\"https://i.scdn.co/image/ab67616d0000b27393432e914046a003229378da\"},{\"height\":300,\"width\":300,\"url\":\"https://i.scdn.co/image/ab67616d00001e0293432e914046a003229378da\"},{\"height\":64,\"width\":64,\"url\":\"https://i.scdn.co/image/ab67616d0000485193432e914046a003229378da\"}],\"external_urls\":{\"spotify\":\"https://open.spotify.com/album/2LkWHNNHgD6BRNeZI2SL1L\"},\"release_date\":\"2014-06-27\",\"release_date_precision\":\"day\"},\"external_ids\":{\"isrc\":\"GBUM71401926\"},\"popularity\":69,\"is_playable\":true,\"linked_from\":null,\"artists\":[{\"name\":\"5 Seconds of Summer\",\"id\":\"5Rl15oVamLq7FbSb0NNBNy\",\"uri\":\"spotify:artist:5Rl15oVamLq7FbSb0NNBNy\",\"href\":\"https://api.spotify.com/v1/artists/5Rl15oVamLq7FbSb0NNBNy\",\"external_urls\":{\"spotify\":\"https://open.spotify.com/artist/5Rl15oVamLq7FbSb0NNBNy\"}}],\"available_markets\":null,\"disc_number\":1,\"duration_ms\":237247,\"explicit\":false,\"external_urls\":{\"spotify\":\"https://open.spotify.com/track/1JCCdiru7fhstOIF4N7WJC\"},\"href\":\"https://api.spotify.com/v1/tracks/1JCCdiru7fhstOIF4N7WJC\",\"id\":\"1JCCdiru7fhstOIF4N7WJC\",\"name\":\"Amnesia\",\"preview_url\":\"\",\"track_number\":12,\"uri\":\"spotify:track:1JCCdiru7fhstOIF4N7WJC\"}}}");
        let data1 = res1.unwrap();
        assert!(&data1.spotify_data.is_some());
        assert_eq!(&data1.artist, "5 Seconds Of Summer");
        assert_eq!(&data1.title, "Amnesia");
        assert_eq!(&data1.info_link, "https://lis.tn/WSKAzD");

        let res2 = parse_response("{\"status\":\"success\",\"result\":{\"artist\":\"Alessia Cara\",\"title\":\"Scars To Your Beautiful\",\"album\":\"Know-It-All\",\"release_date\":\"2015-11-13\",\"label\":\"EP Entertainment, LLC / Def Jam\",\"timecode\":\"00:28\",\"song_link\":\"https://lis.tn/ScarsToYourBeautiful\",\"spotify\":{\"album\":{\"name\":\"Know-It-All (Deluxe)\",\"artists\":[{\"name\":\"Alessia Cara\",\"id\":\"2wUjUUtkb5lvLKcGKsKqsR\",\"uri\":\"spotify:artist:2wUjUUtkb5lvLKcGKsKqsR\",\"href\":\"https://api.spotify.com/v1/artists/2wUjUUtkb5lvLKcGKsKqsR\",\"external_urls\":{\"spotify\":\"https://open.spotify.com/artist/2wUjUUtkb5lvLKcGKsKqsR\"}}],\"album_group\":\"\",\"album_type\":\"album\",\"id\":\"3rDbA12I5duZnlwakqDdZa\",\"uri\":\"spotify:album:3rDbA12I5duZnlwakqDdZa\",\"available_markets\":null,\"href\":\"https://api.spotify.com/v1/albums/3rDbA12I5duZnlwakqDdZa\",\"images\":[{\"height\":640,\"width\":640,\"url\":\"https://i.scdn.co/image/ab67616d0000b273e3ae597159d6c2541c4ee61b\"},{\"height\":300,\"width\":300,\"url\":\"https://i.scdn.co/image/ab67616d00001e02e3ae597159d6c2541c4ee61b\"},{\"height\":64,\"width\":64,\"url\":\"https://i.scdn.co/image/ab67616d00004851e3ae597159d6c2541c4ee61b\"}],\"external_urls\":{\"spotify\":\"https://open.spotify.com/album/3rDbA12I5duZnlwakqDdZa\"},\"release_date\":\"2015-11-13\",\"release_date_precision\":\"day\"},\"external_ids\":{\"isrc\":\"USUM71506811\"},\"popularity\":75,\"is_playable\":true,\"linked_from\":null,\"artists\":[{\"name\":\"Alessia Cara\",\"id\":\"2wUjUUtkb5lvLKcGKsKqsR\",\"uri\":\"spotify:artist:2wUjUUtkb5lvLKcGKsKqsR\",\"href\":\"https://api.spotify.com/v1/artists/2wUjUUtkb5lvLKcGKsKqsR\",\"external_urls\":{\"spotify\":\"https://open.spotify.com/artist/2wUjUUtkb5lvLKcGKsKqsR\"}}],\"available_markets\":null,\"disc_number\":1,\"duration_ms\":230226,\"explicit\":false,\"external_urls\":{\"spotify\":\"https://open.spotify.com/track/0prNGof3XqfTvNDxHonvdK\"},\"href\":\"https://api.spotify.com/v1/tracks/0prNGof3XqfTvNDxHonvdK\",\"id\":\"0prNGof3XqfTvNDxHonvdK\",\"name\":\"Scars To Your Beautiful\",\"preview_url\":\"\",\"track_number\":10,\"uri\":\"spotify:track:0prNGof3XqfTvNDxHonvdK\"}}}");
        let data2 = res2.unwrap();
        assert!(&data2.spotify_data.is_some());
        assert_eq!(&data2.artist, "Alessia Cara");
        assert_eq!(&data2.title, "Scars To Your Beautiful");
        assert_eq!(&data2.info_link, "https://lis.tn/ScarsToYourBeautiful");

        let res3 = parse_response("{\"status\":\"success\",\"result\":{\"artist\":\"Daniel Boone\",\"title\":\"Beautiful Sunday\",\"album\":\"Pop Legend Vol.1\",\"release_date\":\"2010-01-15\",\"label\":\"Open Records\",\"timecode\":\"00:33\",\"song_link\":\"https://lis.tn/YTuccJ\",\"spotify\":{\"album\":{\"name\":\"Cocktail Super Pop\",\"artists\":[{\"name\":\"Various Artists\",\"id\":\"0LyfQWJT6nXafLPZqxe9Of\",\"uri\":\"spotify:artist:0LyfQWJT6nXafLPZqxe9Of\",\"href\":\"https://api.spotify.com/v1/artists/0LyfQWJT6nXafLPZqxe9Of\",\"external_urls\":{\"spotify\":\"https://open.spotify.com/artist/0LyfQWJT6nXafLPZqxe9Of\"}}],\"album_group\":\"\",\"album_type\":\"compilation\",\"id\":\"1ZsLymIsvlHEnGtQFen5xd\",\"uri\":\"spotify:album:1ZsLymIsvlHEnGtQFen5xd\",\"available_markets\":null,\"href\":\"https://api.spotify.com/v1/albums/1ZsLymIsvlHEnGtQFen5xd\",\"images\":[{\"height\":640,\"width\":640,\"url\":\"https://i.scdn.co/image/ab67616d0000b273db8f64a52a4ec4cde9a9528a\"},{\"height\":300,\"width\":300,\"url\":\"https://i.scdn.co/image/ab67616d00001e02db8f64a52a4ec4cde9a9528a\"},{\"height\":64,\"width\":64,\"url\":\"https://i.scdn.co/image/ab67616d00004851db8f64a52a4ec4cde9a9528a\"}],\"external_urls\":{\"spotify\":\"https://open.spotify.com/album/1ZsLymIsvlHEnGtQFen5xd\"},\"release_date\":\"2013-01-18\",\"release_date_precision\":\"day\"},\"external_ids\":{\"isrc\":\"ES5530914999\"},\"popularity\":0,\"is_playable\":true,\"linked_from\":null,\"artists\":[{\"name\":\"Daniel Boone\",\"id\":\"3M5aUsJmembbwKbUx434lS\",\"uri\":\"spotify:artist:3M5aUsJmembbwKbUx434lS\",\"href\":\"https://api.spotify.com/v1/artists/3M5aUsJmembbwKbUx434lS\",\"external_urls\":{\"spotify\":\"https://open.spotify.com/artist/3M5aUsJmembbwKbUx434lS\"}}],\"available_markets\":null,\"disc_number\":1,\"duration_ms\":176520,\"explicit\":false,\"external_urls\":{\"spotify\":\"https://open.spotify.com/track/6o3AMOtlfI6APSUooekMtt\"},\"href\":\"https://api.spotify.com/v1/tracks/6o3AMOtlfI6APSUooekMtt\",\"id\":\"6o3AMOtlfI6APSUooekMtt\",\"name\":\"Beautiful Sunday\",\"preview_url\":\"https://p.scdn.co/mp3-preview/b2fa24732fe08a251b0c8d44774f37fd55378378?cid=e44e7b8278114c7db211c00ea273ac69\",\"track_number\":16,\"uri\":\"spotify:track:6o3AMOtlfI6APSUooekMtt\"}}}");
        let data3 = res3.unwrap();
        assert!(&data3.spotify_data.is_some());
        assert_eq!(&data3.artist, "Daniel Boone");
        assert_eq!(&data3.title, "Beautiful Sunday");
        assert_eq!(&data3.info_link, "https://lis.tn/YTuccJ");
    }
}
