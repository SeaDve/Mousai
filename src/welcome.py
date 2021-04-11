# welcome.py
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

from .window import MousaiWindow


@Gtk.Template(resource_path='/io/github/seadve/Mousai/welcome.ui')
class WelcomeWindow(Adw.ApplicationWindow):
    __gtype_name__ = 'WelcomeWindow'

    submit_button = Gtk.Template.Child()
    token_entry = Gtk.Template.Child()

    def __init__(self, settings, **kwargs):
        super().__init__(**kwargs)
        self.settings = settings
        self.token_entry.set_text(self.settings.get_string("token-value"))
        self.submit_button.connect("clicked", self.on_submit_button_clicked)

    def on_submit_button_clicked(self, widget):
        self.settings.set_string("token-value", self.token_entry.get_text())
        win = MousaiWindow(self.settings, application=self.get_application())
        self.destroy()
        win.present()
