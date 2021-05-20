# utils.py
#
# Copyright 2021 SeaDve
#
# This program is free software: you can redistribute it and/or modify
# it under the terms of the GNU General Public License as published by
# the Free Software Foundation, either version 3 of the License, or
# (at your option) any later version.
#
# This program is distributed in the hope that it will be useful,
# but WITHOUT ANY WARRANTY; without even the implied warranty of
# MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
# GNU General Public License for more details.
#
# You should have received a copy of the GNU General Public License
# along with this program.  If not, see <http://www.gnu.org/licenses/>.

import requests
from subprocess import PIPE, Popen

import gi
gi.require_version('GstPbutils', '1.0')
from gi.repository import GLib, Gst, GstPbutils, GObject

Gst.init(None)


class VoiceRecorder(GObject.GObject):
    __gsignals__ = {'record-done': (GObject.SIGNAL_RUN_LAST, None, ())}

    peak = GObject.Property(type=float, flags=GObject.ParamFlags.READWRITE)

    def __init__(self):
        super().__init__()

        self.pipeline = Gst.Pipeline()
        self.src = Gst.ElementFactory.make('pulsesrc')
        audio_convert = Gst.ElementFactory.make('audioconvert')
        caps = Gst.Caps.from_string('audio/x-raw')
        self.level = Gst.ElementFactory.make('level')
        self.encodebin = Gst.ElementFactory.make('encodebin')
        self.filesink = Gst.ElementFactory.make('filesink')

        self.pipeline.add(self.src)
        self.pipeline.add(audio_convert)
        self.pipeline.add(self.level)
        self.pipeline.add(self.encodebin)
        self.pipeline.add(self.filesink)

        self.src.link(audio_convert)
        audio_convert.link_filtered(self.level, caps)

    def start(self):
        self.record_bus = self.pipeline.get_bus()
        self.record_bus.add_signal_watch()
        self.handler_id = self.record_bus.connect('message', self._on_gst_message)

        self.src.set_property('device', self.get_default_audio_input())
        self.encodebin.set_property('profile', self.get_profile())
        self.filesink.set_property('location', f'{self.get_tmp_dir()}mousaitmp.ogg')
        self.level.link(self.encodebin)
        self.encodebin.link(self.filesink)

        self.pipeline.set_state(Gst.State.PLAYING)

        self.timer = Timer(self.stop)
        # self.timer.start(5)
        self.timer.start(1)

    def cancel(self):
        self.pipeline.set_state(Gst.State.NULL)
        self.record_bus.remove_watch()
        self.record_bus.disconnect(self.handler_id)
        self.timer.cancel()

    def stop(self):
        self.pipeline.set_state(Gst.State.NULL)
        self.record_bus.remove_watch()
        self.record_bus.disconnect(self.handler_id)
        self.emit('record-done')

    def _on_gst_message(self, bus, message):
        t = message.type
        if t == Gst.MessageType.ELEMENT:
            peak = message.get_structure().get_value('peak')[0]
            self.set_property('peak', peak)
        elif t == Gst.MessageType.EOS:
            self.stop()
        elif t == Gst.MessageType.ERROR:
            err, debug = message.parse_error()
            print("Error: %s" % err, debug)

    @staticmethod
    def get_profile():
        audio_caps = Gst.Caps.from_string('audio/x-opus')
        encoding_profile = GstPbutils.EncodingAudioProfile.new(audio_caps, None, None, 1)
        container_caps = Gst.Caps.from_string('application/ogg')
        container_profile = GstPbutils.EncodingContainerProfile.new('record', None, container_caps, None)
        container_profile.add_profile(encoding_profile)
        return container_profile

    @staticmethod
    def get_tmp_dir():
        directory = GLib.getenv('XDG_CACHE_HOME')
        if not directory:
            directory = ""
        return f"{directory}/tmp/"

    @staticmethod
    def get_default_audio_input():
        pactl_output = Popen(
            'pactl info | tail -n +14 | cut -d" " -f3',
            shell=True,
            text=True,
            stdout=PIPE
        ).stdout.read().rstrip()
        return pactl_output

    @staticmethod
    def guess_song(song_file, token):
        data = {'api_token': token, 'return': 'spotify'}
        files = {'file': open(song_file, 'rb')}
        # return requests.post('https://api.audd.io/', data=data, files=files).json()
        return {"status":"success","result":{"artist":"Imagine Dragons","title":"Warriors","album":"Warriors","release_date":"2014-09-18","label":"Universal Music","timecode":"00:40","song_link":"https://lis.tn/Warriors","apple_music":{"previews":[{"url":"https://audio-ssl.itunes.apple.com/itunes-assets/AudioPreview118/v4/65/07/f5/6507f5c5-dba8-f2d5-d56b-39dbb62a5f60/mzaf_1124211745011045566.plus.aac.p.m4a"}],"artwork":{"width":1500,"height":1500,"url":"https://is2-ssl.mzstatic.com/image/thumb/Music128/v4/f4/78/f5/f478f58e-97cf-83b5-b5da-03d31f14e648/00602547623805.rgb.jpg/{w}x{h}bb.jpeg","bgColor":"7f5516","textColor1":"ffe2aa","textColor2":"f8e0bd","textColor3":"e5c58c","textColor4":"e0c59c"},"artistName":"Imagine Dragons","url":"https://music.apple.com/us/album/warriors/1440831203?i=1440831624","discNumber":1,"genreNames":["Alternative","Music"],"durationInMillis":170799,"releaseDate":"2014-09-18","name":"Warriors","isrc":"USUM71414163","albumName":"Smoke + Mirrors (Deluxe)","playParams":{"id":"1440831624","kind":"song"},"trackNumber":18,"composerName":"Imagine Dragons, Alex Da Kid & Josh Mosser"},"spotify":{"album":{"album_type":"album","artists":[{"external_urls":{"spotify":"https://open.spotify.com/artist/53XhwfbYqKCa1cC15pYq2q"},"href":"https://api.spotify.com/v1/artists/53XhwfbYqKCa1cC15pYq2q","id":"53XhwfbYqKCa1cC15pYq2q","name":"Imagine Dragons","type":"artist","uri":"spotify:artist:53XhwfbYqKCa1cC15pYq2q"}],"available_markets":None,"external_urls":{"spotify":"https://open.spotify.com/album/6ecx4OFG0nlUMqAi9OXQER"},"href":"https://api.spotify.com/v1/albums/6ecx4OFG0nlUMqAi9OXQER","id":"6ecx4OFG0nlUMqAi9OXQER","images":[{"height":640,"url":"https://i.scdn.co/image/d3acaeb069f37d8e257221f7224c813c5fa6024e","width":640},{"height":300,"url":"https://i.scdn.co/image/b039549954758689330893bd4a92585092a81cf5","width":300},{"height":64,"url":"https://i.scdn.co/image/67407947517062a649d86e06c7fa17670f7f09eb","width":64}],"name":"Smoke + Mirrors (Deluxe)","release_date":"2015-10-30","release_date_precision":"day","total_tracks":21,"type":"album","uri":"spotify:album:6ecx4OFG0nlUMqAi9OXQER"},"artists":[{"external_urls":{"spotify":"https://open.spotify.com/artist/53XhwfbYqKCa1cC15pYq2q"},"href":"https://api.spotify.com/v1/artists/53XhwfbYqKCa1cC15pYq2q","id":"53XhwfbYqKCa1cC15pYq2q","name":"Imagine Dragons","type":"artist","uri":"spotify:artist:53XhwfbYqKCa1cC15pYq2q"}],"available_markets":None,"disc_number":1,"duration_ms":170066,"explicit":False,"external_ids":{"isrc":"USUM71414163"},"external_urls":{"spotify":"https://open.spotify.com/track/1lgN0A2Vki2FTON5PYq42m"},"href":"https://api.spotify.com/v1/tracks/1lgN0A2Vki2FTON5PYq42m","id":"1lgN0A2Vki2FTON5PYq42m","is_local":False,"name":"Warriors","popularity":66,"track_number":18,"type":"track","uri":"spotify:track:1lgN0A2Vki2FTON5PYq42m"}}}


class Timer:
    def __init__(self, function):
        self.function = function
        self.cancelled = False

    def _update_time(self):
        if self.time_delay == 10 or self.cancelled:
            if not self.cancelled:
                self.function()
            return False
        self.time_delay -= 10
        return True

    def start(self, time_delay):
        self.time_delay = time_delay * 100
        GLib.timeout_add(100, self._update_time)

    def cancel(self):
        self.cancelled = True
