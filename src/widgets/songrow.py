# songrow.py
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

from gi.repository import GdkPixbuf, Gio, Gtk, Adw

from mousai.backend.utils import VoiceRecorder


@Gtk.Template(resource_path='/io/github/seadve/Mousai/ui/songrow.ui')
class SongRow(Adw.ActionRow):
    __gtype_name__ = 'SongRow'

    song_icon = Gtk.Template.Child()

    def __init__(self, song_title, artist, song_link):
        super().__init__()

        self.props.title = song_title
        self.props.subtitle = artist
        self.song_link = song_link
        self.add_prefix(self.song_icon)

        try:
            icon_dir = f"{VoiceRecorder.get_tmp_dir()}{song_title}{artist}.jpg"
            image = GdkPixbuf.Pixbuf.new_from_file(icon_dir)
            self.song_icon.set_loadable_icon(image)
        except Exception:
            pass

    @Gtk.Template.Callback()
    def on_play_button_clicked(self, widget):
        Gio.AppInfo.launch_default_for_uri(self.song_link)
