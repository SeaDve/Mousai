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

import urllib.request

from gi.repository import GdkPixbuf, GLib, Gtk, Adw

from mousai.songrow import SongRow
from mousai.utils import VoiceRecorder


# GTK 4 BLOCKERS
# Use try else
# Reduce Gtk Template Child usage
# Loadable icon for AdwAvatar
# Listbox no get children (Use listview)
# Broken error message
# Icon for welcome window
# Linked entry in welcome window


@Gtk.Template(resource_path='/io/github/seadve/Mousai/ui/window.ui')
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
        self.voice_recorder.connect('record-done', self.on_record_done)
        self.voice_recorder.connect('notify::peak', self.on_peak_changed)
        self.memory_list = list(self.settings.get_value("memory-list"))

        if self.memory_list:
            self.load_memory_list(self.memory_list)
        else:
            self.main_stack.set_visible_child(self.empty_state_box)

    @Gtk.Template.Callback()
    def on_start_button_clicked(self, button):
        self.voice_recorder.start()
        self.main_stack.set_visible_child(self.recording_box)
        self.listen_cancel_stack.set_visible_child(self.cancel_button)

    @Gtk.Template.Callback()
    def on_cancel_button_clicked(self, button):
        self.voice_recorder.cancel()
        self.return_default_page()

    @Gtk.Template.Callback()
    def on_quit(self, window):
        self.settings.set_value("memory-list", GLib.Variant('aa{ss}', self.memory_list))

    def on_peak_changed(self, recorder, peak):
        peak = recorder.peak
        if -9 <= peak <= 0:
            icon_name = "microphone-sensitivity-high-symbolic"
        elif -10 <= peak <= -2:
            icon_name = "microphone-sensitivity-medium-symbolic"
        elif -349 <= peak <= -16:
            icon_name = "microphone-sensitivity-low-symbolic"
        else:
            icon_name = "microphone-sensitivity-muted-symbolic"
        self.recording_box.set_icon_name(icon_name)

    def on_record_done(self, recorder):
        song_file = f"{self.voice_recorder.get_tmp_dir()}mousaitmp.ogg"
        token = self.settings.get_string("token-value")
        output = self.voice_recorder.guess_song(song_file, token)
        status = output["status"]

        print(output)

        try:
            title = output["result"]["title"]
            artist = output["result"]["artist"]
            song_link = output["result"]["song_link"]

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
                                      type=Gtk.MessageType.WARNING,
                                      buttons=Gtk.ButtonsType.OK,
                                      text=_("Sorry!"))
            if status == "error":
                error.format_secondary_text(output["error"]["error_message"])
            elif status == "success" and not output["result"]:
                error.format_secondary_text(_("The song was not recognized."))
            else:
                error.format_secondary_text(_("Something went wrong."))
            error.run()
            error.destroy()

        try:
            icon_uri = output["result"]["spotify"]["album"]["images"][2]["url"]
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
