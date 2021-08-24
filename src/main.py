# SPDX-FileCopyrightText: Copyright 2021 SeaDve
# SPDX-License-Identifier: GPL-3.0-or-later

import sys

import gi
gi.require_version('Gtk', '4.0')
gi.require_version('Gst', '1.0')
gi.require_version('Adw', '1')
from gi.repository import Gtk, Gio, GLib, Adw, Gst

from mousai.widgets.main_window import MainWindow
from mousai.widgets.token_dialog import TokenDialog

Gst.init(None)


class Application(Adw.Application):
    def __init__(self, version):
        super().__init__(application_id='io.github.seadve.Mousai',
                         flags=Gio.ApplicationFlags.FLAGS_NONE)

        self.version = version

        GLib.set_application_name("Mousai")
        GLib.set_prgname('io.github.seadve.Mousai')

    def do_startup(self):
        Adw.Application.do_startup(self)

        self.settings = Gio.Settings.new('io.github.seadve.Mousai')
        self.setup_actions()

    def do_activate(self):
        win = self.props.active_window
        if not win:
            win = MainWindow(self.settings, application=self)
            if not self.settings.get_string('token-value') and \
               not self.settings.get_boolean('dont-show-token-dialog'):
                GLib.timeout_add(5, self.show_token_window)
        win.present()

    def setup_actions(self):
        action = Gio.SimpleAction.new('show-token', None)
        action.connect('activate', self.show_token_window)
        self.add_action(action)

        action = Gio.SimpleAction.new('show-about', None)
        action.connect('activate', self.show_about_dialog)
        self.add_action(action)

        action = Gio.SimpleAction.new('quit', None)
        action.connect('activate', self.on_quit)
        self.add_action(action)

        self.set_accels_for_action('win.clear-history', ('<Primary>BackSpace',))
        self.set_accels_for_action('win.show-help-overlay', ('<Primary>question',))
        self.set_accels_for_action('win.toggle-listen', ('<Primary>r',))
        self.set_accels_for_action('app.show-token', ('<Primary>Delete',))
        self.set_accels_for_action('app.quit', ('<Primary>q',))

    def show_token_window(self, action=None, param=None):
        window = TokenDialog(self.settings)
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

    def on_quit(self, action, param):
        self.get_active_window().close()
        self.quit()


def main(version):
    app = Application(version)
    return app.run(sys.argv)
