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

import json
import urllib.request
from gettext import gettext as _

from gi.repository import Gtk, GdkPixbuf, GLib, Adw

from .songrow import SongRow
from .utils import VoiceRecorder


### GTK 4 BLOCKERS
# Loadable icon for AdwAvatar
# Delete event for window
# Listbox no get children
# Broken error message
# Keyboard accelerator
# Icon for welcome window
# Linked entry in welcome window


@Gtk.Template(resource_path='/io/github/seadve/Mousai/window.ui')
class MousaiWindow(Adw.ApplicationWindow):
    __gtype_name__ = 'MousaiWindow'

    listen_cancel_stack = Gtk.Template.Child()
    start_button = Gtk.Template.Child()
    cancel_button = Gtk.Template.Child()
    history_listbox = Gtk.Template.Child()
    main_stack = Gtk.Template.Child()
    main_screen_box = Gtk.Template.Child()
    recording_box = Gtk.Template.Child()
    empty_state_box = Gtk.Template.Child()

    def __init__(self, settings, **kwargs):
        super().__init__(**kwargs)
        self.settings = settings
        self.voice_recorder = VoiceRecorder()
        self.memory_list = list(self.settings.get_value("memory-list"))

        self.start_button.connect("clicked", self.on_start_button_clicked)
        self.cancel_button.connect("clicked", self.on_cancel_button_clicked)
        # self.connect("delete-event", self.on_quit)

        if self.memory_list:
            self.load_memory_list(self.memory_list)
        else:
            self.main_stack.set_visible_child(self.empty_state_box)

    def on_start_button_clicked(self, widget):
        self.voice_recorder.start(self, self.on_microphone_record_callback)
        self.main_stack.set_visible_child(self.recording_box)
        self.listen_cancel_stack.set_visible_child(self.cancel_button)

    def on_cancel_button_clicked(self, widget):
        self.voice_recorder.cancel()
        self.return_default_page()

    def on_microphone_record_callback(self):
        token = self.settings.get_string("token-value")
        song_file = f"{self.voice_recorder.get_tmp_dir()}mousaitmp.ogg"
        json_output = json.loads(self.voice_recorder.guess_song(token, song_file))
        status = json_output["status"]

        print(json_output)

        try:
            title = json_output["result"]["title"]
            artist = json_output["result"]["artist"]
            song_link = json_output["result"]["song_link"]

            song_link_list = [item["song_link"] for item in self.memory_list]
            if song_link in song_link_list:
                for row in self.history_listbox.get_children():
                    self.history_listbox.remove(row)
                song_link_index = song_link_list.index(song_link)
                self.memory_list.pop(song_link_index)
                self.load_memory_list(self.memory_list)

            song_row = SongRow(title, artist, song_link)
            self.history_listbox.insert(song_row, 0)
            song_entry = {"title": title, "artist": artist, "song_link": song_link}
            self.memory_list.append(song_entry)
        except Exception:
            error = Gtk.MessageDialog(transient_for=self,
                                      buttons=Gtk.ButtonsType.OK,
                                      text=_("Sorry!"))
            if status == "error":
                error.format_secondary_text(json_output["error"]["error_message"])
            elif status == "success" and not json_output["result"]:
                error.format_secondary_text(_("The song was not recognized."))
            else:
                error.format_secondary_text(_("Something went wrong."))
            error.run()
            error.destroy()

        try:
            icon_uri = json_output["result"]["spotify"]["album"]["images"][2]["url"]
            icon_dir = f"{self.voice_recorder.get_tmp_dir()}{title}{artist}.jpg"
            urllib.request.urlretrieve(icon_uri, icon_dir)
            image = GdkPixbuf.Pixbuf.new_from_file(icon_dir)
            song_row.song_icon.set_loadable_icon(image)
        except Exception:
            pass

        self.return_default_page()

    def return_default_page(self):
        if self.memory_list:
            self.main_stack.set_visible_child(self.main_screen_box)
        else:
            self.main_stack.set_visible_child(self.empty_state_box)
        self.listen_cancel_stack.set_visible_child(self.start_button)

    def load_memory_list(self, memory_list):
        for song in memory_list:
            song_row = SongRow(song["title"], song["artist"], song["song_link"])
            self.history_listbox.insert(song_row, 0)

    def clear_memory_list(self):
        self.settings.set_value("memory-list", GLib.Variant('aa{ss}', []))
        for row in self.history_listbox.get_children():
            self.history_listbox.remove(row)
            self.memory_list = []
        self.main_stack.set_visible_child(self.empty_state_box)

    def on_quit(self, widget, param):
        self.settings.set_value("memory-list", GLib.Variant('aa{ss}', self.memory_list))
