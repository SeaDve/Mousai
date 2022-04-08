use async_trait::async_trait;
use gtk::glib;
use rand::seq::SliceRandom;

use std::time::Duration;

use super::{AudD, Data, Error, Provider, ProviderError, Response};
use crate::{core::AudioRecording, model::Song};

#[derive(Debug)]
pub struct AudDMock;

impl AudDMock {
    fn random_data(&self) -> Result<Data, Error> {
        let raw_responses = [
            r#"{"status":"success","result":null}"#,
            r#"{"status":"error","error":{"error_code":901,"error_message":"Recognition failed: authorization failed: no api_token passed and the limit was reached. Get an api_token from dashboard.audd.io."},"request_params":{},"request_api_method":"recognize","request_http_method":"POST","see api documentation":"https://docs.audd.io","contact us":"api@audd.io"}"#,
            r#"{"status":"error","error":{"error_code":900,"error_message":"Recognition failed: authorization failed: wrong api_token. Please check if your account is activated on dashboard.audd.io and has either a trial or an active subscription."},"request_params":{},"request_api_method":"recognize","request_http_method":"POST","see api documentation":"https://docs.audd.io","contact us":"api@audd.io"}"#,
            r#"{"status":"error","error":{"error_code":300,"error_message":"Recognition failed: a problem with fingerprints creating. Keep in mind that you should send only audio files or links to audio files. We support some of the Instagram, Twitter, TikTok and Facebook videos, and also parse html for OpenGraph and JSON-LD media and \\u003caudio\\u003e/\\u003cvideo\\u003e tags, but it's always better to send a 10-20 seconds-long audio file. For audio streams, see https://docs.audd.io/streams/"},"request_params":{},"request_api_method":"recognize","request_http_method":"POST","see api documentation":"https://docs.audd.io","contact us":"api@audd.io"}"#,
            r#"{"status":"success","result":{"artist":"The London Symphony Orchestra","title":"Eine Kleine Nachtmusik","album":"An Hour Of The London Symphony Orchestra","release_date":"2014-04-22","label":"Glory Days Music","timecode":"00:24","song_link":"https://lis.tn/EineKleineNachtmusik"}}"#,
            r#"{"status":"success","result":{"artist":"Public","title":"Make You Mine","album":"Let's Make It","release_date":"2014-10-07","label":"PUBLIC","timecode":"00:43","song_link":"https://lis.tn/FUYgUV"}}"#,
            r#"{"status":"success","result":{"artist":"5 Seconds Of Summer","title":"Amnesia","album":"Amnesia","release_date":"2014-06-24","label":"Universal Music","timecode":"01:02","song_link":"https://lis.tn/WSKAzD","spotify":{"album":{"name":"5 Seconds Of Summer","artists":[{"name":"5 Seconds of Summer","id":"5Rl15oVamLq7FbSb0NNBNy","uri":"spotify:artist:5Rl15oVamLq7FbSb0NNBNy","href":"https://api.spotify.com/v1/artists/5Rl15oVamLq7FbSb0NNBNy","external_urls":{"spotify":"https://open.spotify.com/artist/5Rl15oVamLq7FbSb0NNBNy"}}],"album_group":"","album_type":"album","id":"2LkWHNNHgD6BRNeZI2SL1L","uri":"spotify:album:2LkWHNNHgD6BRNeZI2SL1L","available_markets":null,"href":"https://api.spotify.com/v1/albums/2LkWHNNHgD6BRNeZI2SL1L","images":[{"height":640,"width":640,"url":"https://i.scdn.co/image/ab67616d0000b27393432e914046a003229378da"},{"height":300,"width":300,"url":"https://i.scdn.co/image/ab67616d00001e0293432e914046a003229378da"},{"height":64,"width":64,"url":"https://i.scdn.co/image/ab67616d0000485193432e914046a003229378da"}],"external_urls":{"spotify":"https://open.spotify.com/album/2LkWHNNHgD6BRNeZI2SL1L"},"release_date":"2014-06-27","release_date_precision":"day"},"external_ids":{"isrc":"GBUM71401926"},"popularity":69,"is_playable":true,"linked_from":null,"artists":[{"name":"5 Seconds of Summer","id":"5Rl15oVamLq7FbSb0NNBNy","uri":"spotify:artist:5Rl15oVamLq7FbSb0NNBNy","href":"https://api.spotify.com/v1/artists/5Rl15oVamLq7FbSb0NNBNy","external_urls":{"spotify":"https://open.spotify.com/artist/5Rl15oVamLq7FbSb0NNBNy"}}],"available_markets":null,"disc_number":1,"duration_ms":237247,"explicit":false,"external_urls":{"spotify":"https://open.spotify.com/track/1JCCdiru7fhstOIF4N7WJC"},"href":"https://api.spotify.com/v1/tracks/1JCCdiru7fhstOIF4N7WJC","id":"1JCCdiru7fhstOIF4N7WJC","name":"Amnesia","preview_url":"","track_number":12,"uri":"spotify:track:1JCCdiru7fhstOIF4N7WJC"}}}"#,
            r#"{"status":"success","result":{"artist":"Alessia Cara","title":"Scars To Your Beautiful","album":"Know-It-All","release_date":"2015-11-13","label":"EP Entertainment, LLC / Def Jam","timecode":"00:28","song_link":"https://lis.tn/ScarsToYourBeautiful","spotify":{"album":{"name":"Know-It-All (Deluxe)","artists":[{"name":"Alessia Cara","id":"2wUjUUtkb5lvLKcGKsKqsR","uri":"spotify:artist:2wUjUUtkb5lvLKcGKsKqsR","href":"https://api.spotify.com/v1/artists/2wUjUUtkb5lvLKcGKsKqsR","external_urls":{"spotify":"https://open.spotify.com/artist/2wUjUUtkb5lvLKcGKsKqsR"}}],"album_group":"","album_type":"album","id":"3rDbA12I5duZnlwakqDdZa","uri":"spotify:album:3rDbA12I5duZnlwakqDdZa","available_markets":null,"href":"https://api.spotify.com/v1/albums/3rDbA12I5duZnlwakqDdZa","images":[{"height":640,"width":640,"url":"https://i.scdn.co/image/ab67616d0000b273e3ae597159d6c2541c4ee61b"},{"height":300,"width":300,"url":"https://i.scdn.co/image/ab67616d00001e02e3ae597159d6c2541c4ee61b"},{"height":64,"width":64,"url":"https://i.scdn.co/image/ab67616d00004851e3ae597159d6c2541c4ee61b"}],"external_urls":{"spotify":"https://open.spotify.com/album/3rDbA12I5duZnlwakqDdZa"},"release_date":"2015-11-13","release_date_precision":"day"},"external_ids":{"isrc":"USUM71506811"},"popularity":75,"is_playable":true,"linked_from":null,"artists":[{"name":"Alessia Cara","id":"2wUjUUtkb5lvLKcGKsKqsR","uri":"spotify:artist:2wUjUUtkb5lvLKcGKsKqsR","href":"https://api.spotify.com/v1/artists/2wUjUUtkb5lvLKcGKsKqsR","external_urls":{"spotify":"https://open.spotify.com/artist/2wUjUUtkb5lvLKcGKsKqsR"}}],"available_markets":null,"disc_number":1,"duration_ms":230226,"explicit":false,"external_urls":{"spotify":"https://open.spotify.com/track/0prNGof3XqfTvNDxHonvdK"},"href":"https://api.spotify.com/v1/tracks/0prNGof3XqfTvNDxHonvdK","id":"0prNGof3XqfTvNDxHonvdK","name":"Scars To Your Beautiful","preview_url":"","track_number":10,"uri":"spotify:track:0prNGof3XqfTvNDxHonvdK"}}}"#,
            r#"{"status":"success","result":{"artist":"Daniel Boone","title":"Beautiful Sunday","album":"Pop Legend Vol.1","release_date":"2010-01-15","label":"Open Records","timecode":"00:33","song_link":"https://lis.tn/YTuccJ","spotify":{"album":{"name":"Cocktail Super Pop","artists":[{"name":"Various Artists","id":"0LyfQWJT6nXafLPZqxe9Of","uri":"spotify:artist:0LyfQWJT6nXafLPZqxe9Of","href":"https://api.spotify.com/v1/artists/0LyfQWJT6nXafLPZqxe9Of","external_urls":{"spotify":"https://open.spotify.com/artist/0LyfQWJT6nXafLPZqxe9Of"}}],"album_group":"","album_type":"compilation","id":"1ZsLymIsvlHEnGtQFen5xd","uri":"spotify:album:1ZsLymIsvlHEnGtQFen5xd","available_markets":null,"href":"https://api.spotify.com/v1/albums/1ZsLymIsvlHEnGtQFen5xd","images":[{"height":640,"width":640,"url":"https://i.scdn.co/image/ab67616d0000b273db8f64a52a4ec4cde9a9528a"},{"height":300,"width":300,"url":"https://i.scdn.co/image/ab67616d00001e02db8f64a52a4ec4cde9a9528a"},{"height":64,"width":64,"url":"https://i.scdn.co/image/ab67616d00004851db8f64a52a4ec4cde9a9528a"}],"external_urls":{"spotify":"https://open.spotify.com/album/1ZsLymIsvlHEnGtQFen5xd"},"release_date":"2013-01-18","release_date_precision":"day"},"external_ids":{"isrc":"ES5530914999"},"popularity":0,"is_playable":true,"linked_from":null,"artists":[{"name":"Daniel Boone","id":"3M5aUsJmembbwKbUx434lS","uri":"spotify:artist:3M5aUsJmembbwKbUx434lS","href":"https://api.spotify.com/v1/artists/3M5aUsJmembbwKbUx434lS","external_urls":{"spotify":"https://open.spotify.com/artist/3M5aUsJmembbwKbUx434lS"}}],"available_markets":null,"disc_number":1,"duration_ms":176520,"explicit":false,"external_urls":{"spotify":"https://open.spotify.com/track/6o3AMOtlfI6APSUooekMtt"},"href":"https://api.spotify.com/v1/tracks/6o3AMOtlfI6APSUooekMtt","id":"6o3AMOtlfI6APSUooekMtt","name":"Beautiful Sunday","preview_url":"https://p.scdn.co/mp3-preview/b2fa24732fe08a251b0c8d44774f37fd55378378?cid=e44e7b8278114c7db211c00ea273ac69","track_number":16,"uri":"spotify:track:6o3AMOtlfI6APSUooekMtt"}}}"#,
        ];

        let random_response = raw_responses
            .choose(&mut rand::thread_rng())
            .expect("Failed to get choose random from raw responses");

        log::debug!("random_response: {}", random_response);

        Ok(Response::parse(random_response.as_bytes())?.data()?)
    }
}

#[async_trait(?Send)]
impl Provider for AudDMock {
    async fn recognize(&self, _: &AudioRecording) -> Result<Song, ProviderError> {
        glib::timeout_future(Duration::from_secs(1)).await;
        Ok(AudD::handle_data(
            self.random_data().map_err(ProviderError::AudD)?,
        ))
    }

    fn listen_duration(&self) -> Duration {
        Duration::from_secs(1)
    }
}
