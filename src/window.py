# window.py
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
import json
from subprocess import PIPE, Popen

from gi.repository import Gtk, Gst, GLib

Gst.init(None)


@Gtk.Template(resource_path='/io/github/seadve/Mousai/window.ui')
class MousaiWindow(Gtk.ApplicationWindow):
    __gtype_name__ = 'MousaiWindow'

    start_button = Gtk.Template.Child()

    def __init__(self, **kwargs):
        super().__init__(**kwargs)

        self.start_button.connect("clicked", self.on_start_button_clicked)

        self.voice_recorder = VoiceRecorder()

    def on_start_button_clicked(self, widget):

        self.voice_recorder.start()



class VoiceRecorder:
    def start(self):
        pipeline = f'pulsesrc device="{self.get_default_audio_input()}" ! audioconvert ! opusenc ! webmmux ! filesink location={self.get_tmp_dir()}'
        self.recorder_gst = Gst.parse_launch(pipeline)
        bus = self.recorder_gst.get_bus()
        bus.add_signal_watch()
        bus.connect("message", self._on_recorder_gst_message)
        self.recorder_gst.set_state(Gst.State.PLAYING)

        timer = Timer(self._on_stop_record, 5)
        timer.start()

    def _on_stop_record(self):
        self.recorder_gst.send_event(Gst.Event.new_eos())
        self.callback()

    def callback(self):

        song_file = self.get_tmp_dir()

        result = self.song_guesser(song_file)

        test = json.loads(result)

        print(test["status"])
        try:
            print(test["result"]["title"])
            print(test["result"]["artist"])
        except Exception:
            print(test)
            print("Song not found")

    def song_guesser(self, song_file):
        data = {
            'return': 'apple_music,spotify',
            'api_token': 'e49148ca676e38f5c8d3d47feac62af8'
        }

        files = {
            'file': open(song_file, 'rb'),
        }

        result = requests.post('https://api.audd.io/', data=data, files=files)
        return result.text


    def _on_recorder_gst_message(self, bus, message):
        t = message.type
        if t == Gst.MessageType.EOS:
            self.recorder_gst.set_state(Gst.State.NULL)
        elif t == Gst.MessageType.ERROR:
            self.recorder_gst.set_state(Gst.State.NULL)
            err, debug = message.parse_error()
            print("Error: %s" % err, debug)

    def get_tmp_dir(self):
        directory = GLib.getenv('XDG_CACHE_HOME')
        if not directory:
            directory = ""
        #return f"{directory}/tmp/mousaitmp.ogg"
        return "/home/dave/test.ogg"

    def get_default_audio_input(self):
        pactl_output = Popen(
            'pactl info | tail -n +14 | cut -d" " -f3',
            shell=True,
            text=True,
            stdout=PIPE
        ).stdout.read().rstrip()
        return pactl_output


class Timer:
    def __init__(self, function, time_delay):
        self.function = function
        self.time_delay = time_delay * 100

    def _displaydelay(self):
        if self.time_delay == 10: #or self.stopped:
            self.function()
            return False
        self.time_delay -= 10
        print(self.time_delay)
        return True

    def start(self):
        GLib.timeout_add(100, self._displaydelay)

    def stop(self):
        self.stopped = True

    
