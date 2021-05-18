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
from gi.repository import GLib, Gst, GstPbutils

Gst.init(None)


class VoiceRecorder:
    def __init__(self):
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

    def start(self, window, param):
        self.window = window

        self.record_bus = self.pipeline.get_bus()
        self.record_bus.add_signal_watch()
        self.handler_id = self.record_bus.connect('message', self._on_gst_message)

        self.src.set_property('device', self.get_default_audio_input())
        self.encodebin.set_property('profile', self.get_profile())
        self.filesink.set_property('location', f'{self.get_tmp_dir()}mousaitmp.ogg')
        self.level.link(self.encodebin)
        self.encodebin.link(self.filesink)

        self.pipeline.set_state(Gst.State.PLAYING)

        self.timer = Timer(self.stop, param, 5)
        self.timer.start()

    def cancel(self):
        self.pipeline.set_state(Gst.State.NULL)
        self.record_bus.remove_watch()
        self.record_bus.disconnect(self.handler_id)
        self.timer.stop()

    def stop(self, callback):
        self.pipeline.set_state(Gst.State.NULL)
        self.record_bus.remove_watch()
        self.record_bus.disconnect(self.handler_id)
        callback()

    def _on_gst_message(self, bus, message):
        t = message.type
        if t == Gst.MessageType.ELEMENT:
            try:
                val = message.get_structure().get_value("peak")[0]
            except (AttributeError, TypeError, IndexError):
                pass
            else:
                if -9 <= val <= 0:
                    self.window.recording_box.set_icon_name("microphone-sensitivity-high-symbolic")
                elif -10 <= val <= -2:
                    self.window.recording_box.set_icon_name("microphone-sensitivity-medium-symbolic")
                elif -349 <= val <= -16:
                    self.window.recording_box.set_icon_name("microphone-sensitivity-low-symbolic")
                elif val < -349:
                    self.window.recording_box.set_icon_name("microphone-sensitivity-muted-symbolic")
                    self.window.recording_box.set_title("Muted")

                if not val >= 1000:
                    self.window.recording_box.set_title("Listening")
        elif t == Gst.MessageType.EOS:
            self.stop()
        elif t == Gst.MessageType.ERROR:
            err, debug = message.parse_error()
            print("Error: %s" % err, debug)

    def get_profile(self):
        audio_caps = Gst.Caps.from_string('audio/x-opus')
        audio_caps.set_value('channels', 1)

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
        return requests.post('https://api.audd.io/', data=data, files=files).json()


class Timer:
    def __init__(self, function, param, time_delay):
        self.function = function
        self.param = param
        self.time_delay = time_delay * 100
        self.stopped = False

    def _displaydelay(self):
        if self.time_delay == 10 or self.stopped:
            if not self.stopped:
                self.function(self.param)
            return False
        self.time_delay -= 10
        return True

    def start(self):
        GLib.timeout_add(100, self._displaydelay)

    def stop(self):
        self.stopped = True
