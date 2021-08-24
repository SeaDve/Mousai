# SPDX-FileCopyrightText: Copyright 2021 SeaDve
# SPDX-License-Identifier: GPL-3.0-or-later

import threading

from gi.repository import GLib, Gtk, Adw, Gio, Gst

from mousai.widgets.song import Song
from mousai.widgets.song_row import SongRow
from mousai.backend.voice_recorder import VoiceRecorder
from mousai.backend.utils import Utils
from mousai.backend.song_player import SongPlayer


@Gtk.Template(resource_path='/io/github/seadve/Mousai/ui/main_window.ui')
class MainWindow(Adw.ApplicationWindow):
    __gtype_name__ = 'MainWindow'

    main_stack = Gtk.Template.Child()
    recording_box = Gtk.Template.Child()
    history_listbox = Gtk.Template.Child()

    def __init__(self, settings, **kwargs):
        super().__init__(**kwargs)
        self.settings = settings

        self.set_default_icon_name('io.github.seadve.Mousai')

        self.history_model = Gio.ListStore.new(Song)
        self.history_listbox.bind_model(self.history_model, self.new_song_row)

        self.song_player = SongPlayer()

        self.voice_recorder = VoiceRecorder()
        self.voice_recorder.connect('record-done', self.on_record_done)
        self.voice_recorder.connect('notify::peak', self.on_peak_changed)

        self.setup_actions()
        self.load_window_size()
        self.load_history()
        self.return_default_page()

    def do_close_request(self):
        songs_list = [dict(song) for song in self.history_model]
        self.settings.set_value('memory-list', GLib.Variant('aa{ss}', songs_list))
        self.save_window_size()

    def setup_actions(self):
        action = Gio.SimpleAction.new('clear-history', None)
        action.connect('activate', lambda *_: self.clear_history())
        self.add_action(action)

        action = Gio.SimpleAction.new('toggle-listen', None)
        action.connect('activate', self.on_toggle_listen)
        self.add_action(action)

        action = self.settings.create_action('preferred-audio-source')
        self.add_action(action)

    def new_song_row(self, song):
        song_row = SongRow(song)
        song_row.connect('notify::is-playing', self.on_song_row_notify_is_playing)
        self.song_player.connect('stopped', song_row.on_song_player_stopped)
        return song_row

    def return_default_page(self):
        if self.history_model:
            self.main_stack.set_visible_child_name('main-screen')
        else:
            self.main_stack.set_visible_child_name('empty-state')
        self.lookup_action('clear-history').set_enabled(True)

    def load_history(self):
        for saved_songs in list(self.settings.get_value('memory-list')):
            song = Song(*saved_songs.values())
            self.history_model.append(song)

    def remove_duplicates(self, song_id):
        for index, song in enumerate(self.history_model):
            if song.song_link == song_id:
                self.history_model.remove(index)
                break

    def clear_history(self):
        self.history_model.remove_all()
        self.main_stack.set_visible_child_name('empty-state')
        self.song_player.stop()

    def load_window_size(self):
        size = self.settings.get_value('window-size')
        self.set_default_size(*size)

    def save_window_size(self):
        size = self.get_width(), self.get_height()
        self.settings.set_value('window-size', GLib.Variant('ai', [*size]))

    def on_song_row_notify_is_playing(self, song_row, pspec):
        # Signal other buttons to stop before playing
        self.song_player.stop()

        if song_row.props.is_playing:
            self.song_player.play(song_row.song_src)

    def on_peak_changed(self, recorder, peak):
        peak = recorder.peak
        if -6 < peak <= 0:
            icon_name = 'microphone-sensitivity-high-symbolic'
            title = _("Listening")
        elif -15 < peak <= -6:
            icon_name = 'microphone-sensitivity-medium-symbolic'
            title = _("Listening")
        elif -349 < peak <= -15:
            icon_name = 'microphone-sensitivity-low-symbolic'
            title = _("Listening")
        else:
            icon_name = 'microphone-sensitivity-muted-symbolic'
            title = _("Muted")

        self.recording_box.set_icon_name(icon_name)
        self.recording_box.set_title(title)

    def on_record_done(self, recorder, highest_peak):
        if highest_peak < -349:
            self.show_error(_("No audio detected"), _("Please check your audio device."))
            self.return_default_page()
            return

        song_file = f'{Utils.get_tmp_dir()}/mousaitmp.ogg'
        token = self.settings.get_string('token-value')
        thread = threading.Thread(target=self.guess_song, args=(song_file, token))
        thread.start()

    def on_toggle_listen(self, action, param):
        if self.voice_recorder.props.state == Gst.State.NULL:
            self.voice_recorder.start(self.get_preferred_default_audio_source())
            self.main_stack.set_visible_child_name('recording')
            self.lookup_action('clear-history').set_enabled(False)
            self.song_player.stop()
        else:
            self.voice_recorder.cancel()
            self.return_default_page()

    def show_error(self, title, subtitle):
        error = Gtk.MessageDialog(transient_for=self, modal=True,
                                  buttons=Gtk.ButtonsType.OK, title=title)
        error.props.text = subtitle
        error.present()
        error.connect('response', lambda *_: error.close())

    def get_preferred_default_audio_source(self):
        preferred_audio_source = self.settings.get_string('preferred-audio-source')
        default_speaker, default_mic = Utils.get_default_audio_sources()

        if preferred_audio_source == "mic":
            return default_mic
        else:
            return default_speaker

    def guess_song(self, song_file, token):
        output, image_src = Utils.guess_song(song_file, token)
        GLib.idle_add(self.update_history, output, image_src)

    def update_history(self, output, image_src):
        status = output['status']

        try:
            result = output['result']
            song = Song(*result.values())
        except (AttributeError, KeyError):
            if status == 'error':
                error_subtitle = output['error_message']
            elif status == 'success' and not result:
                error_subtitle = _("The song was not recognized.")
            else:
                error_subtitle = _("Something went wrong.")

            self.show_error(_("Sorry!"), error_subtitle)
        else:
            if image_src:
                icon_dir = f'{Utils.get_tmp_dir()}/{song.title}{song.artist}.jpg'
                Utils.download_image(image_src, icon_dir)

            self.remove_duplicates(song.song_link)
            self.history_model.insert(0, song)

        self.return_default_page()

    @Gtk.Template.Callback()
    def get_visible_button(self, window, visible_child):
        return 'cancel' if visible_child == 'recording' else 'listen'
