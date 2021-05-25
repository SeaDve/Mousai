# main_window.py
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

from gi.repository import GLib, Gtk, Adw, Gio

from mousai.widgets.song import Song
from mousai.widgets.song_row import SongRow
from mousai.backend.voice_recorder import VoiceRecorder
from mousai.backend.utils import Utils


@Gtk.Template(resource_path='/io/github/seadve/Mousai/ui/main_window.ui')
class MainWindow(Adw.ApplicationWindow):
    __gtype_name__ = 'MainWindow'

    main_stack = Gtk.Template.Child()
    recording_box = Gtk.Template.Child()
    history_listbox = Gtk.Template.Child()

    def __init__(self, settings, **kwargs):
        super().__init__(**kwargs)
        self.settings = settings

        self.history_model = Gio.ListStore.new(Song)
        self.history_listbox.bind_model(self.history_model, self.new_song_row)

        self.voice_recorder = VoiceRecorder()
        self.voice_recorder.connect('record-done', self.on_record_done)
        self.voice_recorder.connect('notify::peak', self.on_peak_changed)

        self.setup_actions()
        self.load_window_size()
        self.load_history()
        self.return_default_page()

    def setup_actions(self):
        action = Gio.SimpleAction.new('clear-history', None)
        action.connect('activate', lambda *_: self.clear_history())
        self.add_action(action)

        action = Gio.SimpleAction.new('quit', None)
        action.connect('activate', lambda *_: self.close())
        self.add_action(action)

    def new_song_row(self, song):
        song_row = SongRow(song)
        self.main_stack.connect('notify::visible-child-name', song_row.on_window_recording)
        return song_row

    def remove_duplicates(self, song_id):
        for index, song in enumerate(self.history_model):
            if song.song_link == song_id:
                self.history_model.remove(index)
                break

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

    def clear_history(self):
        self.history_model.remove_all()
        self.main_stack.set_visible_child_name('empty-state')

    def save_window_size(self):
        size = (
            self.get_size(Gtk.Orientation.HORIZONTAL),
            self.get_size(Gtk.Orientation.VERTICAL)
        )
        self.settings.set_value('window-size', GLib.Variant('ai', [*size]))

    def load_window_size(self):
        size = self.settings.get_value('window-size')
        self.set_default_size(*size)

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

    def on_record_done(self, recorder):
        song_file = f'{Utils.get_tmp_dir()}/mousaitmp.ogg'
        token = self.settings.get_string('token-value')
        output, image_src = Utils.guess_song(song_file, token)
        status = output['status']

        try:
            result = output['result']
            song = Song(*result.values())
        except (AttributeError, KeyError):
            error = Gtk.MessageDialog(transient_for=self, modal=True,
                                      buttons=Gtk.ButtonsType.OK, title=_("Sorry"))
            if status == 'error':
                error.props.text = output['error_message']
            elif status == 'success' and not result:
                error.props.text = _("The song was not recognized.")
            else:
                error.props.text = _("Something went wrong.")
            error.present()
            error.connect('response', lambda *_: error.close())
        else:
            if image_src:
                icon_dir = f'{Utils.get_tmp_dir()}/{song.title}{song.artist}.jpg'
                Utils.download_image(image_src, icon_dir)

            self.remove_duplicates(song.song_link)
            self.history_model.insert(0, song)

        self.return_default_page()

    @Gtk.Template.Callback()
    def on_quit(self, window):
        songs_list = [dict(song) for song in self.history_model]
        self.settings.set_value('memory-list', GLib.Variant('aa{ss}', songs_list))
        self.save_window_size()

    @Gtk.Template.Callback()
    def on_start_button_clicked(self, button):
        self.voice_recorder.start()
        self.main_stack.set_visible_child_name('recording')
        self.lookup_action('clear-history').set_enabled(False)

    @Gtk.Template.Callback()
    def on_cancel_button_clicked(self, button):
        self.voice_recorder.cancel()
        self.return_default_page()

    @Gtk.Template.Callback()
    def get_visible_button(self, window, visible_child):
        return 'cancel' if visible_child == 'recording' else 'listen'
