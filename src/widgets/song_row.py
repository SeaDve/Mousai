# SPDX-FileCopyrightText: Copyright 2021 SeaDve
# SPDX-License-Identifier: GPL-3.0-or-later

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
        self.song_icon.set_custom_image(self.get_song_icon())

    def get_song_icon(self):
        path = f'{Utils.get_tmp_dir()}/{self.props.title}{self.props.subtitle}.jpg'
        file = Gio.File.new_for_path(path)
        try:
            return Gdk.Texture.new_from_file(file)
        except GLib.Error:
            return None

    def on_window_recording(self, stack, _):
        if self.button_player.is_stopped and stack.get_visible_child_name() == 'recording':
            self.button_player.is_stopped = False

    @Gtk.Template.Callback()
    def on_open_link_button_clicked(self, button):
        Gio.AppInfo.launch_default_for_uri(self.song_link)

    @Gtk.Template.Callback()
    def get_playback_icon(self, self_, is_stopped):
        return 'media-playback-stop-symbolic' if is_stopped else 'media-playback-start-symbolic'
