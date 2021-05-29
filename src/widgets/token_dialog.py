# SPDX-FileCopyrightText: Copyright 2021 SeaDve
# SPDX-License-Identifier: GPL-3.0-or-later

from gi.repository import Gtk


@Gtk.Template(resource_path='/io/github/seadve/Mousai/ui/token_dialog.ui')
class TokenDialog(Gtk.Dialog):
    __gtype_name__ = 'TokenDialog'

    token_entry = Gtk.Template.Child()
    submit_button = Gtk.Template.Child()

    def __init__(self, settings):
        super().__init__()
        self.settings = settings
        self.token_entry.set_text(self.settings.get_string('token-value'))

        # Workaround to hide titlebar
        placeholder = Gtk.Box()
        placeholder.set_visible(False)
        self.set_titlebar(placeholder)

    @Gtk.Template.Callback()
    def on_submit_button_clicked(self, _):
        if self.submit_button.get_sensitive():
            self.settings.set_string('token-value', self.token_entry.get_text())
            self.close()

    @Gtk.Template.Callback()
    def on_text_changed(self, entry):
        if clickable := entry.get_text_length() in (0, 32):
            entry.remove_css_class('error')
        else:
            entry.add_css_class('error')
        self.submit_button.set_sensitive(clickable)
