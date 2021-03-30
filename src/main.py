# main.py
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

import sys
import gi

gi.require_version('Gtk', '3.0')
gi.require_version('Gst', '1.0')
gi.require_version('Handy', '1')

from gi.repository import Gtk, Gio, Handy, Gdk, GLib

from .window import MousaiWindow
from .welcome import WelcomeWindow


class Application(Gtk.Application):
    def __init__(self, version):
        super().__init__(application_id='io.github.seadve.Mousai',
                         flags=Gio.ApplicationFlags.FLAGS_NONE)

        self.version = version

        GLib.set_application_name("Mousai")
        GLib.set_prgname('io.github.seadve.Mousai')

    def do_startup(self):
        Gtk.Application.do_startup(self)

        css_provider = Gtk.CssProvider()
        css_provider.load_from_resource('/io/github/seadve/Mousai/style.css')
        screen = Gdk.Screen.get_default()
        Gtk.StyleContext.add_provider_for_screen(
            screen, css_provider, Gtk.STYLE_PROVIDER_PRIORITY_APPLICATION,
        )

        self.settings = Gio.Settings.new('io.github.seadve.Mousai')

        Handy.init()

    def do_activate(self):
        token = self.settings.get_string("token-value")
        win = self.props.active_window
        if not win:
            if token == "default":
                win = WelcomeWindow(self.settings, application=self)
            else:
                win = MousaiWindow(self.settings, application=self)
        win.present()


def main(version):
    app = Application(version)
    return app.run(sys.argv)
