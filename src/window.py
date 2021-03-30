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

import os
import json
import requests
import time
import urllib.request
from subprocess import PIPE, Popen

from gi.repository import Gtk, Gst, GLib, Handy, Gio, GdkPixbuf

Gst.init(None)

# DONE Implement song not found
# DONE add empty state on main_screen
# DONE save song history
# DONE add image on action row
# DONE make it easier to insert token
# DONE move to proper data dir

# use hdy avatar
# fix issue on first start
# DONE make listening state more beautiful
# more informative insert token state

@Gtk.Template(resource_path='/io/github/seadve/Mousai/window.ui')
class MousaiWindow(Handy.ApplicationWindow):
    __gtype_name__ = 'MousaiWindow'

    listen_cancel_stack = Gtk.Template.Child()
    start_button = Gtk.Template.Child()
    cancel_button = Gtk.Template.Child()
    history_listbox = Gtk.Template.Child()

    main_stack = Gtk.Template.Child()
    main_screen_box = Gtk.Template.Child()
    recording_box = Gtk.Template.Child()
    empty_state_box = Gtk.Template.Child()

    progressbar =  Gtk.Template.Child()
    progressbar1 =  Gtk.Template.Child()

    def __init__(self, settings, **kwargs):
        super().__init__(**kwargs)
        self.settings = settings
        self.start_button.connect("clicked", self.on_start_button_clicked)
        self.cancel_button.connect("clicked", self.on_cancel_button_clicked)
        self.connect("delete-event", self.on_quit)
        self.voice_recorder = VoiceRecorder()

        self.json_directory = f"{self.voice_recorder.get_tmp_dir()}mousai.json"

        try:
            with open(self.json_directory, "r") as memory_file:
                self.memory_list = json.load(memory_file)
        except Exception:
            with open(self.json_directory, "w") as memory_file:
                memory_file.write("[]") # WILL CREATE ERROR ON FIRST LAUNCH
                self.memory_list = json.load(memory_file)

        if self.memory_list:
            self.load_memory_list(self.memory_list)
        else:
            self.main_stack.set_visible_child(self.empty_state_box)

    def load_memory_list(self, memory_list):
        for num, item in enumerate(memory_list):
            title = memory_list[num]["title"]
            artist = memory_list[num]["artist"]
            song_link = memory_list[num]["song_link"]
            #icon_uri = memory_list[num]["icon_uri"]

            song_row = SongRow(title, artist, song_link)
            song_row.show()
            self.history_listbox.insert(song_row, 0)

    def clear_memory_list(self):
        empty_list = json.dumps([], indent=4)
        with open(self.json_directory, "w") as memory_file:
            memory_file.write(empty_list)
            self.memory_list = []

        win = MousaiWindow(self.settings, application=self.get_application())
        self.destroy()
        win.present()

    def on_quit(self, widget, arg):
        json_memory = json.dumps(self.memory_list, indent=4)
        with open(self.json_directory, "w") as outfile:
            outfile.write(json_memory)

    def on_start_button_clicked(self, widget):
        self.voice_recorder.start(self, self.on_microphone_record_callback)

        self.main_stack.set_visible_child(self.recording_box)
        self.listen_cancel_stack.set_visible_child(self.cancel_button)

    def on_cancel_button_clicked(self, widget):
        self.voice_recorder.cancel()

        if self.memory_list:
            self.main_stack.set_visible_child(self.main_screen_box)
        else:
            self.main_stack.set_visible_child(self.empty_state_box)
        self.listen_cancel_stack.set_visible_child(self.start_button)

    def on_microphone_record_callback(self):
        song_file = f"{self.voice_recorder.get_tmp_dir()}mousaitmp.ogg"
        json_output = json.loads(self.song_guesser(song_file))

        print(json_output)
        print(json_output["status"])

        try:
            title = json_output["result"]["title"]
            artist = json_output["result"]["artist"]
            song_link = json_output["result"]["song_link"]
            icon_uri = json_output["result"]["spotify"]["album"]["images"][2]["url"]

            self.song_entry = {}
            self.song_entry["title"] = title
            self.song_entry["artist"] = artist
            self.song_entry["song_link"] = song_link
            self.song_entry["icon_uri"] = icon_uri
            self.memory_list.append(self.song_entry)

            urllib.request.urlretrieve(icon_uri, f'{title}.jpg')

            song_row = SongRow(title, artist, song_link)
            song_row.song_icon.set_from_file(f"{title}.jpg")
            song_row.show()
            self.history_listbox.insert(song_row, 0)
        except Exception:
            error = Gtk.MessageDialog(transient_for=self,
                                      type=Gtk.MessageType.WARNING,
                                      buttons=Gtk.ButtonsType.OK,
                                      text=_("Sorry!"))
            error.format_secondary_text(_("The song was not recognized."))
            error.run()
            error.destroy()

        if self.memory_list:
            self.main_stack.set_visible_child(self.main_screen_box)
        else:
            self.main_stack.set_visible_child(self.empty_state_box)
        self.listen_cancel_stack.set_visible_child(self.start_button)

    def song_guesser(self, song_file):
        token = self.settings.get_string("token-value")

        data = {
            'api_token': token,
            'return': 'spotify',
        }
        files = {'file': open(song_file, 'rb')}

        result = requests.post('https://api.audd.io/', data=data, files=files)
        return result.text
        #return """{"status": "test"}"""


@Gtk.Template(resource_path='/io/github/seadve/Mousai/songrow.ui')
class SongRow(Handy.ActionRow):
    __gtype_name__ = 'SongRow'

    song_icon = Gtk.Template.Child()

    def __init__(self, title, artist, song_link, **kwargs):
        super().__init__(**kwargs)

        self.set_title(title)
        self.set_subtitle(artist)
        self.song_link = song_link

        #self.song_icon.set_text(title)
        self.song_icon.set_from_file(f"{VoiceRecorder.get_tmp_dir()}{title}.jpg")
        self.add_prefix(self.song_icon)

        placeholder = Gtk.Button()
        self.set_activatable_widget(placeholder)
        self.connect("activated", self.on_songrow_clicked)

    def on_songrow_clicked(self, widget):
        Gio.AppInfo.launch_default_for_uri(self.song_link)


class VoiceRecorder:
    def start(self, window, param):
        self.window = window

        # AUDIO RECORDER
        pipeline = f'pulsesrc device="{self.get_default_audio_input()}" ! audioconvert ! opusenc ! webmmux ! filesink location={self.get_tmp_dir()}mousaitmp.ogg'
        self.recorder_gst = Gst.parse_launch(pipeline)
        bus = self.recorder_gst.get_bus()
        bus.add_signal_watch()
        bus.connect("message", self._on_recorder_gst_message)
        self.recorder_gst.set_state(Gst.State.PLAYING)

        # VISUALIZER
        pipeline = f'pulsesrc device="{self.get_default_audio_input()}" ! audioconvert ! level interval=50000000 ! fakesink qos=false'
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
        try:
            p = message.get_structure().get_value("rms")
            frac = int(50-(-1*p[0]))/50
            self.window.progressbar.set_fraction(frac)
            self.window.progressbar1.set_fraction(frac)
        except:
            pass

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
