# welcome_window.py
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

from gi.repository import Gtk, Adw


@Gtk.Template(resource_path='/io/github/seadve/Mousai/ui/token_dialog.ui')
class TokenDialog(Gtk.Dialog):
    __gtype_name__ = 'TokenDialog'

    token_entry = Gtk.Template.Child()

    def __init__(self, settings):
        super().__init__()
        self.settings = settings
        self.token_entry.set_text(self.settings.get_string('token-value'))

        # Workaround to hide titlebar
        placeholder = Gtk.HeaderBar()
        placeholder.set_visible(False)
        self.set_titlebar(placeholder)

    @Gtk.Template.Callback()
    def on_submit_button_clicked(self, button):
        self.settings.set_string('token-value', self.token_entry.get_text())
        self.close()
