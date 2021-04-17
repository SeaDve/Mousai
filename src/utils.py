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

from gi.repository import GLib, Gst

Gst.init(None)


class VoiceRecorder:
    def start(self, window, param):
        self.window = window

        # AUDIO RECORDER
        pipeline = (f'pulsesrc device="{self.get_default_audio_input()}" ! audioconvert ! '
                    f'opusenc ! webmmux ! filesink location={self.get_tmp_dir()}mousaitmp.ogg')
        self.recorder_gst = Gst.parse_launch(pipeline)
        bus = self.recorder_gst.get_bus()
        bus.add_signal_watch()
        bus.connect("message", self._on_recorder_gst_message)
        self.recorder_gst.set_state(Gst.State.PLAYING)

        # VISUALIZER
        pipeline = (f'pulsesrc device="{self.get_default_audio_input()}" ! audioconvert ! '
                    'level interval=50000000 ! fakesink qos=false')
        self.visualizer_gst = Gst.parse_launch(pipeline)
        bus = self.visualizer_gst.get_bus()
        bus.add_signal_watch()
        bus.connect("message", self._on_visualizer_gst_message)
        self.visualizer_gst.set_state(Gst.State.PLAYING)

        self.timer = Timer(self._on_stop_record, param, 5)
        self.timer.start()

    def cancel(self):
        self.recorder_gst.send_event(Gst.Event.new_eos())
        self.visualizer_gst.send_event(Gst.Event.new_eos())
        self.visualizer_gst.set_state(Gst.State.NULL)
        self.timer.stop()

    def _on_stop_record(self, callback):
        self.recorder_gst.send_event(Gst.Event.new_eos())
        self.visualizer_gst.send_event(Gst.Event.new_eos())
        self.visualizer_gst.set_state(Gst.State.NULL)
        callback()

    def _on_recorder_gst_message(self, bus, message):
        t = message.type
        if t == Gst.MessageType.EOS:
            self.recorder_gst.set_state(Gst.State.NULL)
        elif t == Gst.MessageType.ERROR:
            self.recorder_gst.set_state(Gst.State.NULL)
            err, debug = message.parse_error()
            print("Error: %s" % err, debug)

    def _on_visualizer_gst_message(self, bus, message):
        val = 100
        try:
            p = message.get_structure().get_value("rms")
            val = int(p[0] * -2.2)
        except Exception:
            pass

        if 0 <= val <= 36:
            self.window.recording_box.set_icon_name("microphone-sensitivity-high-symbolic")
        elif 37 <= val <= 57:
            self.window.recording_box.set_icon_name("microphone-sensitivity-medium-symbolic")
        elif 58 <= val <= 85:
            self.window.recording_box.set_icon_name("microphone-sensitivity-low-symbolic")
        elif val >= 86:
            self.window.recording_box.set_icon_name("microphone-sensitivity-muted-symbolic")

        if val >= 100:
            self.window.recording_box.set_title("Muted")
        else:
            self.window.recording_box.set_title("Listening")

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
