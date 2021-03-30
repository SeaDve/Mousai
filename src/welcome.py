from gi.repository import Gtk, Handy

from .window import MousaiWindow

@Gtk.Template(resource_path='/io/github/seadve/Mousai/welcome.ui')
class WelcomeWindow(Handy.ApplicationWindow):
    __gtype_name__ = 'WelcomeWindow'

    submit_button = Gtk.Template.Child()
    token_entry = Gtk.Template.Child()

    def __init__(self, settings, **kwargs):
        super().__init__(**kwargs)
        self.settings = settings

        self.submit_button.connect("clicked", self.on_submit_button_clicked)

    def on_submit_button_clicked(self, widget):
        token = self.token_entry.get_text()
        self.settings.set_string("token-value", token)
        win = MousaiWindow(self.settings, application=self.get_application())
        win.present()
        self.destroy()
