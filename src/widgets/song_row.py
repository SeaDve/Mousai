# song_row.py
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

from gi.repository import Gio, Gtk, Adw, GLib, Gdk

from mousai.backend.utils import Utils
from mousai.widgets.button_player import ButtonPlayer  # noqa: F401


@Gtk.Template(resource_path='/io/github/seadve/Mousai/ui/song_row.ui')
class SongRow(Adw.ActionRow):
    __gtype_name__ = 'SongRow'

    song_icon = Gtk.Template.Child()
    button_player = Gtk.Template.Child()

    def __init__(self, song):
        super().__init__()

        self.props.title = song.title
        self.props.subtitle = song.artist
        self.song_link = song.song_link
        self.song_src = song.song_src

        self.button_player.set_song_src(self.song_src)
        self.add_prefix(self.song_icon)
        self.song_icon.set_custom_image(self.load_song_icon())

    def load_song_icon(self):
        path = f'{Utils.get_tmp_dir()}/{self.props.title}{self.props.subtitle}.jpg'
        file = Gio.File.new_for_path(path)
        try:
            pixbuf = Gdk.Texture.new_from_file(file)
        except GLib.Error:
            return None
        return pixbuf

    def on_window_recording(self, stack, _):
        if self.button_player.is_stopped and stack.get_visible_child_name() == 'recording':
            self.button_player.is_stopped = False

    @Gtk.Template.Callback()
    def on_open_link_button_clicked(self, button):
        Gio.AppInfo.launch_default_for_uri(self.song_link)

    @Gtk.Template.Callback()
    def get_playback_icon(self, self_, is_stopped):
        return 'media-playback-stop-symbolic' if is_stopped else 'media-playback-start-symbolic'
