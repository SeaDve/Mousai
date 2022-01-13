use async_trait::async_trait;
use bytes::Bytes;
use reqwest::Client;
use serde::Deserialize;
use serde_json::json;

use std::time::Duration;

use super::Provider;
use crate::{core::AudioRecording, model::Song, utils, RUNTIME};

#[derive(Debug, Deserialize)]
pub struct Image {
    pub url: String,
}

#[derive(Debug, Deserialize)]
pub struct Album {
    pub images: Vec<Image>,
}

#[derive(Debug, Deserialize)]
pub struct SpotifyData {
    pub album: Album,
    pub disc_number: u32,
    pub track_number: u32,
}

#[derive(Debug, Deserialize)]
pub struct Data {
    pub title: String,
    pub artist: String,
    pub timecode: String,
    #[serde(rename(deserialize = "song_link"))]
    pub info_link: String,
    #[serde(rename(deserialize = "spotify"))]
    pub spotify_data: SpotifyData,
}

#[derive(Debug, Deserialize)]
pub struct Response {
    #[serde(rename(deserialize = "result"))]
    pub data: Option<Data>,
    pub status: String,
}

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

    fn handle_json(json_bytes: &Bytes) -> anyhow::Result<Song> {
        let response: Response = serde_json::from_slice(json_bytes)?;

        anyhow::ensure!(
            response.status == "success",
            "Returned {} as status",
            response.status
        );

        let data = response
            .data
            .ok_or(anyhow::anyhow!("Cannot recognize song"))?;

        Ok(Song::new(&data.title, &data.artist, &data.info_link))
    }
}

#[async_trait(?Send)]
impl Provider for AudD {
    async fn recognize(&self, recording: &AudioRecording) -> anyhow::Result<Song> {
        let data = json!({
            "api_token": self.api_token,
            "return": "spotify",
            "audio": utils::file_to_base64(recording.path()).await?,
        });

        let server_response = RUNTIME
            .spawn(
                Client::new()
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

        Self::handle_json(&bytes)
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

    // TODO implement test using `handle_response` method

    #[test]
    fn not_recognized() {
        "{\"status\":\"success\",\"result\":null}";
    }

    #[test]
    fn daily_limit_reached() {
        "{\"status\":\"error\",\"error\":{\"error_code\":901,\"error_message\":\"Recognition failed: authorization failed: no api_token passed and the limit was reached. Get an api_token from dashboard.audd.io.\"},\"request_params\":{},\"request_api_method\":\"recognize\",\"request_http_method\":\"POST\",\"see api documentation\":\"https://docs.audd.io\",\"contact us\":\"api@audd.io\"}";
    }

    #[test]
    fn wrong_api_token() {
        "{\"status\":\"error\",\"error\":{\"error_code\":900,\"error_message\":\"Recognition failed: authorization failed: wrong api_token. Please check if your account is activated on dashboard.audd.io and has either a trial or an active subscription.\"},\"request_params\":{},\"request_api_method\":\"recognize\",\"request_http_method\":\"POST\",\"see api documentation\":\"https://docs.audd.io\",\"contact us\":\"api@audd.io\"}";
    }

    #[test]
    fn wrong_file_sent_or_audio_without_streams() {
        // TODO test if actually no streams
        "{\"status\":\"error\",\"error\":{\"error_code\":300,\"error_message\":\"Recognition failed: a problem with fingerprints creating. Keep in mind that you should send only audio files or links to audio files. We support some of the Instagram, Twitter, TikTok and Facebook videos, and also parse html for OpenGraph and JSON-LD media and \\u003caudio\\u003e/\\u003cvideo\\u003e tags, but it's always better to send a 10-20 seconds-long audio file. For audio streams, see https://docs.audd.io/streams/\"},\"request_params\":{},\"request_api_method\":\"recognize\",\"request_http_method\":\"POST\",\"see api documentation\":\"https://docs.audd.io\",\"contact us\":\"api@audd.io\"}";
    }

    #[test]
    fn proper_but_no_spotify_field() {
        "{\"status\":\"success\",\"result\":{\"artist\":\"The London Symphony Orchestra\",\"title\":\"Eine Kleine Nachtmusik\",\"album\":\"An Hour Of The London Symphony Orchestra\",\"release_date\":\"2014-04-22\",\"label\":\"Glory Days Music\",\"timecode\":\"00:24\",\"song_link\":\"https://lis.tn/EineKleineNachtmusik\"}}";
        "{\"status\":\"success\",\"result\":{\"artist\":\"Public\",\"title\":\"Make You Mine\",\"album\":\"Let's Make It\",\"release_date\":\"2014-10-07\",\"label\":\"PUBLIC\",\"timecode\":\"00:43\",\"song_link\":\"https://lis.tn/FUYgUV\"}}";
    }

    #[test]
    fn proper() {
        "{\"status\":\"success\",\"result\":{\"artist\":\"5 Seconds Of Summer\",\"title\":\"Amnesia\",\"album\":\"Amnesia\",\"release_date\":\"2014-06-24\",\"label\":\"Universal Music\",\"timecode\":\"01:02\",\"song_link\":\"https://lis.tn/WSKAzD\",\"spotify\":{\"album\":{\"name\":\"5 Seconds Of Summer\",\"artists\":[{\"name\":\"5 Seconds of Summer\",\"id\":\"5Rl15oVamLq7FbSb0NNBNy\",\"uri\":\"spotify:artist:5Rl15oVamLq7FbSb0NNBNy\",\"href\":\"https://api.spotify.com/v1/artists/5Rl15oVamLq7FbSb0NNBNy\",\"external_urls\":{\"spotify\":\"https://open.spotify.com/artist/5Rl15oVamLq7FbSb0NNBNy\"}}],\"album_group\":\"\",\"album_type\":\"album\",\"id\":\"2LkWHNNHgD6BRNeZI2SL1L\",\"uri\":\"spotify:album:2LkWHNNHgD6BRNeZI2SL1L\",\"available_markets\":null,\"href\":\"https://api.spotify.com/v1/albums/2LkWHNNHgD6BRNeZI2SL1L\",\"images\":[{\"height\":640,\"width\":640,\"url\":\"https://i.scdn.co/image/ab67616d0000b27393432e914046a003229378da\"},{\"height\":300,\"width\":300,\"url\":\"https://i.scdn.co/image/ab67616d00001e0293432e914046a003229378da\"},{\"height\":64,\"width\":64,\"url\":\"https://i.scdn.co/image/ab67616d0000485193432e914046a003229378da\"}],\"external_urls\":{\"spotify\":\"https://open.spotify.com/album/2LkWHNNHgD6BRNeZI2SL1L\"},\"release_date\":\"2014-06-27\",\"release_date_precision\":\"day\"},\"external_ids\":{\"isrc\":\"GBUM71401926\"},\"popularity\":69,\"is_playable\":true,\"linked_from\":null,\"artists\":[{\"name\":\"5 Seconds of Summer\",\"id\":\"5Rl15oVamLq7FbSb0NNBNy\",\"uri\":\"spotify:artist:5Rl15oVamLq7FbSb0NNBNy\",\"href\":\"https://api.spotify.com/v1/artists/5Rl15oVamLq7FbSb0NNBNy\",\"external_urls\":{\"spotify\":\"https://open.spotify.com/artist/5Rl15oVamLq7FbSb0NNBNy\"}}],\"available_markets\":null,\"disc_number\":1,\"duration_ms\":237247,\"explicit\":false,\"external_urls\":{\"spotify\":\"https://open.spotify.com/track/1JCCdiru7fhstOIF4N7WJC\"},\"href\":\"https://api.spotify.com/v1/tracks/1JCCdiru7fhstOIF4N7WJC\",\"id\":\"1JCCdiru7fhstOIF4N7WJC\",\"name\":\"Amnesia\",\"preview_url\":\"\",\"track_number\":12,\"uri\":\"spotify:track:1JCCdiru7fhstOIF4N7WJC\"}}}";
        "{\"status\":\"success\",\"result\":{\"artist\":\"Alessia Cara\",\"title\":\"Scars To Your Beautiful\",\"album\":\"Know-It-All\",\"release_date\":\"2015-11-13\",\"label\":\"EP Entertainment, LLC / Def Jam\",\"timecode\":\"00:28\",\"song_link\":\"https://lis.tn/ScarsToYourBeautiful\",\"spotify\":{\"album\":{\"name\":\"Know-It-All (Deluxe)\",\"artists\":[{\"name\":\"Alessia Cara\",\"id\":\"2wUjUUtkb5lvLKcGKsKqsR\",\"uri\":\"spotify:artist:2wUjUUtkb5lvLKcGKsKqsR\",\"href\":\"https://api.spotify.com/v1/artists/2wUjUUtkb5lvLKcGKsKqsR\",\"external_urls\":{\"spotify\":\"https://open.spotify.com/artist/2wUjUUtkb5lvLKcGKsKqsR\"}}],\"album_group\":\"\",\"album_type\":\"album\",\"id\":\"3rDbA12I5duZnlwakqDdZa\",\"uri\":\"spotify:album:3rDbA12I5duZnlwakqDdZa\",\"available_markets\":null,\"href\":\"https://api.spotify.com/v1/albums/3rDbA12I5duZnlwakqDdZa\",\"images\":[{\"height\":640,\"width\":640,\"url\":\"https://i.scdn.co/image/ab67616d0000b273e3ae597159d6c2541c4ee61b\"},{\"height\":300,\"width\":300,\"url\":\"https://i.scdn.co/image/ab67616d00001e02e3ae597159d6c2541c4ee61b\"},{\"height\":64,\"width\":64,\"url\":\"https://i.scdn.co/image/ab67616d00004851e3ae597159d6c2541c4ee61b\"}],\"external_urls\":{\"spotify\":\"https://open.spotify.com/album/3rDbA12I5duZnlwakqDdZa\"},\"release_date\":\"2015-11-13\",\"release_date_precision\":\"day\"},\"external_ids\":{\"isrc\":\"USUM71506811\"},\"popularity\":75,\"is_playable\":true,\"linked_from\":null,\"artists\":[{\"name\":\"Alessia Cara\",\"id\":\"2wUjUUtkb5lvLKcGKsKqsR\",\"uri\":\"spotify:artist:2wUjUUtkb5lvLKcGKsKqsR\",\"href\":\"https://api.spotify.com/v1/artists/2wUjUUtkb5lvLKcGKsKqsR\",\"external_urls\":{\"spotify\":\"https://open.spotify.com/artist/2wUjUUtkb5lvLKcGKsKqsR\"}}],\"available_markets\":null,\"disc_number\":1,\"duration_ms\":230226,\"explicit\":false,\"external_urls\":{\"spotify\":\"https://open.spotify.com/track/0prNGof3XqfTvNDxHonvdK\"},\"href\":\"https://api.spotify.com/v1/tracks/0prNGof3XqfTvNDxHonvdK\",\"id\":\"0prNGof3XqfTvNDxHonvdK\",\"name\":\"Scars To Your Beautiful\",\"preview_url\":\"\",\"track_number\":10,\"uri\":\"spotify:track:0prNGof3XqfTvNDxHonvdK\"}}}";
        "{\"status\":\"success\",\"result\":{\"artist\":\"Daniel Boone\",\"title\":\"Beautiful Sunday\",\"album\":\"Pop Legend Vol.1\",\"release_date\":\"2010-01-15\",\"label\":\"Open Records\",\"timecode\":\"00:33\",\"song_link\":\"https://lis.tn/YTuccJ\",\"spotify\":{\"album\":{\"name\":\"Cocktail Super Pop\",\"artists\":[{\"name\":\"Various Artists\",\"id\":\"0LyfQWJT6nXafLPZqxe9Of\",\"uri\":\"spotify:artist:0LyfQWJT6nXafLPZqxe9Of\",\"href\":\"https://api.spotify.com/v1/artists/0LyfQWJT6nXafLPZqxe9Of\",\"external_urls\":{\"spotify\":\"https://open.spotify.com/artist/0LyfQWJT6nXafLPZqxe9Of\"}}],\"album_group\":\"\",\"album_type\":\"compilation\",\"id\":\"1ZsLymIsvlHEnGtQFen5xd\",\"uri\":\"spotify:album:1ZsLymIsvlHEnGtQFen5xd\",\"available_markets\":null,\"href\":\"https://api.spotify.com/v1/albums/1ZsLymIsvlHEnGtQFen5xd\",\"images\":[{\"height\":640,\"width\":640,\"url\":\"https://i.scdn.co/image/ab67616d0000b273db8f64a52a4ec4cde9a9528a\"},{\"height\":300,\"width\":300,\"url\":\"https://i.scdn.co/image/ab67616d00001e02db8f64a52a4ec4cde9a9528a\"},{\"height\":64,\"width\":64,\"url\":\"https://i.scdn.co/image/ab67616d00004851db8f64a52a4ec4cde9a9528a\"}],\"external_urls\":{\"spotify\":\"https://open.spotify.com/album/1ZsLymIsvlHEnGtQFen5xd\"},\"release_date\":\"2013-01-18\",\"release_date_precision\":\"day\"},\"external_ids\":{\"isrc\":\"ES5530914999\"},\"popularity\":0,\"is_playable\":true,\"linked_from\":null,\"artists\":[{\"name\":\"Daniel Boone\",\"id\":\"3M5aUsJmembbwKbUx434lS\",\"uri\":\"spotify:artist:3M5aUsJmembbwKbUx434lS\",\"href\":\"https://api.spotify.com/v1/artists/3M5aUsJmembbwKbUx434lS\",\"external_urls\":{\"spotify\":\"https://open.spotify.com/artist/3M5aUsJmembbwKbUx434lS\"}}],\"available_markets\":null,\"disc_number\":1,\"duration_ms\":176520,\"explicit\":false,\"external_urls\":{\"spotify\":\"https://open.spotify.com/track/6o3AMOtlfI6APSUooekMtt\"},\"href\":\"https://api.spotify.com/v1/tracks/6o3AMOtlfI6APSUooekMtt\",\"id\":\"6o3AMOtlfI6APSUooekMtt\",\"name\":\"Beautiful Sunday\",\"preview_url\":\"https://p.scdn.co/mp3-preview/b2fa24732fe08a251b0c8d44774f37fd55378378?cid=e44e7b8278114c7db211c00ea273ac69\",\"track_number\":16,\"uri\":\"spotify:track:6o3AMOtlfI6APSUooekMtt\"}}}";
        "{\"status\":\"success\",\"result\":{\"artist\":\"Shawn Mendes\",\"title\":\"Higher\",\"album\":\"Wonder\",\"release_date\":\"2020-12-04\",\"label\":\"UMG - Shawn Mendes LP4-5 PS/ Island\",\"timecode\":\"00:28\",\"song_link\":\"https://lis.tn/oBvCpO\",\"spotify\":{\"album\":{\"name\":\"Wonder\",\"artists\":[{\"name\":\"Shawn Mendes\",\"id\":\"7n2wHs1TKAczGzO7Dd2rGr\",\"uri\":\"spotify:artist:7n2wHs1TKAczGzO7Dd2rGr\",\"href\":\"https://api.spotify.com/v1/artists/7n2wHs1TKAczGzO7Dd2rGr\",\"external_urls\":{\"spotify\":\"https://open.spotify.com/artist/7n2wHs1TKAczGzO7Dd2rGr\"}}],\"album_group\":\"\",\"album_type\":\"album\",\"id\":\"3Lp4JKk2ZgNkybMRS3eZR5\",\"uri\":\"spotify:album:3Lp4JKk2ZgNkybMRS3eZR5\",\"available_markets\":null,\"href\":\"https://api.spotify.com/v1/albums/3Lp4JKk2ZgNkybMRS3eZR5\",\"images\":[{\"height\":640,\"width\":640,\"url\":\"https://i.scdn.co/image/ab67616d0000b27337a5a19e52f8260b3b158e55\"},{\"height\":300,\"width\":300,\"url\":\"https://i.scdn.co/image/ab67616d00001e0237a5a19e52f8260b3b158e55\"},{\"height\":64,\"width\":64,\"url\":\"https://i.scdn.co/image/ab67616d0000485137a5a19e52f8260b3b158e55\"}],\"external_urls\":{\"spotify\":\"https://open.spotify.com/album/3Lp4JKk2ZgNkybMRS3eZR5\"},\"release_date\":\"2020-12-04\",\"release_date_precision\":\"day\"},\"external_ids\":{\"isrc\":\"USUM72018804\"},\"popularity\":61,\"is_playable\":true,\"linked_from\":null,\"artists\":[{\"name\":\"Shawn Mendes\",\"id\":\"7n2wHs1TKAczGzO7Dd2rGr\",\"uri\":\"spotify:artist:7n2wHs1TKAczGzO7Dd2rGr\",\"href\":\"https://api.spotify.com/v1/artists/7n2wHs1TKAczGzO7Dd2rGr\",\"external_urls\":{\"spotify\":\"https://open.spotify.com/artist/7n2wHs1TKAczGzO7Dd2rGr\"}}],\"available_markets\":null,\"disc_number\":1,\"duration_ms\":160143,\"explicit\":false,\"external_urls\":{\"spotify\":\"https://open.spotify.com/track/7nuQ1LgM6l5FLaJmlODoqj\"},\"href\":\"https://api.spotify.com/v1/tracks/7nuQ1LgM6l5FLaJmlODoqj\",\"id\":\"7nuQ1LgM6l5FLaJmlODoqj\",\"name\":\"Higher\",\"preview_url\":\"\",\"track_number\":3,\"uri\":\"spotify:track:7nuQ1LgM6l5FLaJmlODoqj\"}}}";
    }
}
