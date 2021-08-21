from gi.repository import Gtk, GObject, Gio, Adw


@Gtk.Template(resource_path='/io/github/seadve/Mousai/ui/playback_indicator.ui')
class PlaybackIndicator(Adw.Bin):
    __gtype_name__ = 'MousaiPlaybackIndicator'

    spinner = Gtk.Template.Child()

    _is_active = False

    def __init__(self):
        super().__init__()

    @GObject.Property(type=bool, default=_is_active)
    def is_active(self):
        return self._is_active

    @is_active.setter  # type: ignore
    def is_active(self, is_active):
        self._is_active = is_active
        if is_active:
            self.spinner.add_css_class('active')
        else:
            self.spinner.remove_css_class('active')
