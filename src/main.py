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
from gi.repository import Gtk, Gdk, Gio, GLib, Adw, Gst

from mousai.widgets.main_window import MainWindow
from mousai.widgets.token_dialog import TokenDialog

Gst.init(None)


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
            win = MainWindow(self.settings, application=self)
            if not self.settings.get_string('token-value'):
                GLib.timeout_add(5, lambda: self.show_token_window())
        win.present()

    def setup_actions(self):
        simple_actions = [
            ('show-token', self.show_token_window, ('<Ctrl>Delete',)),
            ('show-shortcuts', self.show_shortcuts_window, ('<Ctrl>question',)),
            ('show-about', self.show_about_dialog, None),
        ]

        for action, callback, accel in simple_actions:
            simple_action = Gio.SimpleAction.new(action, None)
            simple_action.connect('activate', callback)
            self.add_action(simple_action)
            if accel:
                self.set_accels_for_action(f'app.{action}', accel)

        self.set_accels_for_action('win.clear-history', ('<Ctrl>BackSpace',))
        self.set_accels_for_action('win.quit', ('<Ctrl>q',))

    def show_token_window(self, action=None, param=None):
        window = TokenDialog(self.settings)
        window.set_transient_for(self.get_active_window())
        window.present()

    def show_shortcuts_window(self, action, param):
        builder = Gtk.Builder.new_from_resource('/io/github/seadve/Mousai/ui/shortcuts_window.ui')
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
        about.present()


def main(version):
    app = Application(version)
    return app.run(sys.argv)
