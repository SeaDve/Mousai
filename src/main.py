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
gi.require_version('Gtk', '4.0')
gi.require_version('Gst', '1.0')
gi.require_version('Adw', '1')
from gi.repository import Gtk, Gdk, Gio, GLib, Adw

from mousai.widgets.window import MousaiWindow
from mousai.widgets.welcome import WelcomeWindow


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
        css_provider.load_from_resource('/io/github/seadve/Mousai/ui/style.css')
        display = Gdk.Display.get_default()
        Gtk.StyleContext.add_provider_for_display(
            display, css_provider, Gtk.STYLE_PROVIDER_PRIORITY_APPLICATION,
        )

        self.settings = Gio.Settings.new('io.github.seadve.Mousai')
        self.setup_actions()

        Adw.init()

    def do_activate(self):
        win = self.props.active_window
        if not win:
            if not self.settings.get_string("token-value"):
                win = WelcomeWindow(self.settings, application=self)
            else:
                win = MousaiWindow(self.settings, application=self)
        win.present()

    def setup_actions(self):
        simple_actions = [
            ("clear-history", self.clear_song_history, ("<Ctrl>BackSpace",)),
            ("reset-token", self.reset_token_value, ("<Ctrl>Delete",)),
            ("show-shortcuts", self.show_shortcuts_window, ("<Ctrl>question",)),
            ("show-about", self.show_about_dialog, None),
            ("quit", self.on_quit, ("<Ctrl>q",)),
        ]

        for action, callback, accel in simple_actions:
            simple_action = Gio.SimpleAction.new(action, None)
            simple_action.connect("activate", callback)
            self.add_action(simple_action)
            if accel:
                self.set_accels_for_action(f"app.{action}", accel)

    def clear_song_history(self, action, param):
        self.get_active_window().clear_memory_list()

    def reset_token_value(self, action, param):
        self.get_active_window().on_quit(None)
        self.get_active_window().destroy()
        win = WelcomeWindow(self.settings, application=self)
        win.present()

    def show_shortcuts_window(self, action, param):
        builder = Gtk.Builder()
        builder.add_from_resource('/io/github/seadve/Mousai/ui/shortcuts.ui')
        window = builder.get_object('shortcuts')
        window.set_transient_for(self.get_active_window())
        window.present()

    def show_about_dialog(self, action, param):
        about = Gtk.AboutDialog()
        about.set_transient_for(self.get_active_window())
        about.set_modal(True)
        about.set_version(self.version)
        about.set_program_name("Mousai")
        about.set_logo_icon_name("io.github.seadve.Mousai")
        about.set_authors(["Dave Patrick"])
        about.set_comments(_("Identify any songs in seconds"))
        about.set_wrap_license(True)
        about.set_license_type(Gtk.License.GPL_3_0)
        about.set_copyright(_("Copyright 2021 Dave Patrick"))
        # Translators: Replace "translator-credits" with your names, one name per line
        about.set_translator_credits(_("translator-credits"))
        about.set_website_label(_("GitHub"))
        about.set_website("https://github.com/SeaDve/Mousai")
        about.show()

    def on_quit(self, action, param):
        self.get_active_window().on_quit(None)
        self.quit()


def main(version):
    app = Application(version)
    return app.run(sys.argv)
