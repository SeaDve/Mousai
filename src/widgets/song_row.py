# SPDX-FileCopyrightText: Copyright 2021 SeaDve
# SPDX-License-Identifier: GPL-3.0-or-later

from gi.repository import Gio, Gtk, Adw, GLib, Gdk, GObject

from mousai.backend.utils import Utils


@Gtk.Template(resource_path='/io/github/seadve/Mousai/ui/song_row.ui')
class SongRow(Adw.ActionRow):
    __gtype_name__ = 'SongRow'

    song_icon = Gtk.Template.Child()
    play_pause_button = Gtk.Template.Child()

    _is_playing = False

    def __init__(self, song):
        super().__init__()

        self.props.title = song.title
        self.props.subtitle = song.artist
        self.song_link = song.song_link
        self.song_src = song.song_src

        self.play_pause_button.set_sensitive(self.song_src)
        self.add_prefix(self.song_icon)
        self.song_icon.set_custom_image(self.get_song_icon())

    @GObject.Property(type=bool, default=_is_playing)
    def is_playing(self):
        return self._is_playing

    @is_playing.setter  # type: ignore
    def is_playing(self, is_playing):
        self._is_playing = is_playing
        if is_playing:
            self.song_icon.add_css_class('playing')
        else:
            self.song_icon.remove_css_class('playing')

    def get_song_icon(self):
        path = f'{Utils.get_tmp_dir()}/{self.props.title}{self.props.subtitle}.jpg'
        file = Gio.File.new_for_path(path)
        try:
            return Gdk.Texture.new_from_file(file)
        except GLib.Error:
            return None

    def on_song_player_stopped(self, song_player, song_src):
        if song_src != self.song_src:
            return

        self.props.is_playing = False

    @Gtk.Template.Callback()
    def on_play_pause_button_clicked(self, button):
        self.props.is_playing = not self.props.is_playing

    @Gtk.Template.Callback()
    def on_open_link_button_clicked(self, button):
        Gio.AppInfo.launch_default_for_uri(self.song_link)

    @Gtk.Template.Callback()
    def get_play_pause_button_icon_name(self, button, is_playing):
        return 'media-playback-stop-symbolic' if is_playing else 'media-playback-start-symbolic'
