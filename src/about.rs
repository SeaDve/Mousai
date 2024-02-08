use std::{env, path::Path};

use gettextrs::gettext;
use gtk::{glib, prelude::*};

use crate::config::{APP_ID, VERSION};

pub fn present_window(transient_for: Option<&impl IsA<gtk::Window>>) {
    let win = adw::AboutWindow::builder()
        .modal(true)
        .application_icon(APP_ID)
        .application_name(gettext("Mousai"))
        .developer_name(gettext("Dave Patrick Caberto"))
        .version(VERSION)
        .copyright(gettext("© 2023 Dave Patrick Caberto"))
        .license_type(gtk::License::Gpl30)
        // Translators: Replace "translator-credits" with your names. Put a comma between.
        .translator_credits(gettext("translator-credits"))
        .issue_url("https://github.com/SeaDve/Mousai/issues")
        .support_url("https://github.com/SeaDve/Mousai/discussions")
        .debug_info(debug_info())
        .debug_info_filename("mousai-debug-info")
        .release_notes_version("0.7.0")
        .release_notes(release_notes())
        .build();

    win.add_link(
        &gettext("Donate (Buy Me a Coffee)"),
        "https://www.buymeacoffee.com/seadve",
    );
    win.add_link(&gettext("GitHub"), "https://github.com/SeaDve/Mousai");
    win.add_link(
        &gettext("Translate"),
        "https://hosted.weblate.org/projects/seadve/mousai",
    );

    win.set_transient_for(transient_for);
    win.present();
}

fn debug_info() -> String {
    let is_flatpak = Path::new("/.flatpak-info").exists();

    let language_names = glib::language_names().join(", ");

    let distribution = glib::os_info("PRETTY_NAME").unwrap_or_else(|| "<unknown>".into());
    let desktop_session = env::var("DESKTOP_SESSION").unwrap_or_else(|_| "<unknown>".into());
    let display_server = env::var("XDG_SESSION_TYPE").unwrap_or_else(|_| "<unknown>".into());

    let gtk_version = format!(
        "{}.{}.{}",
        gtk::major_version(),
        gtk::minor_version(),
        gtk::micro_version()
    );
    let adw_version = format!(
        "{}.{}.{}",
        adw::major_version(),
        adw::minor_version(),
        adw::micro_version()
    );
    let soup_version = format!(
        "{}.{}.{}",
        soup::major_version(),
        soup::minor_version(),
        soup::micro_version()
    );
    let gst_version_string = gst::version_string();
    let libpulse_version = pulse::version::get_library_version().to_string_lossy();

    format!(
        r#"- {APP_ID} {VERSION}
- Flatpak: {is_flatpak}

- Language: {language_names}

- Distribution: {distribution}
- Desktop Session: {desktop_session}
- Display Server: {display_server}

- GTK {gtk_version}
- Libadwaita {adw_version}
- Libsoup {soup_version}
- {gst_version_string}
- Libpulse {libpulse_version}"#
    )
}

fn release_notes() -> &'static str {
    r#"<p>This update contains huge UI updates and fixes:</p>
    <ul>
      <li>New feature-rich UI</li>
      <li>Added section to browse song information and checkout providers</li>
      <li>Added offline mode</li>
      <li>Added fuzzy search on the history</li>
      <li>Added MPRIS support</li>
      <li>Added ability to remove individual song from history</li>
      <li>The title and artist of the song can now be copied from the UI</li>
      <li>The player is now seekable</li>
      <li>The recognizing stage is now cancellable</li>
      <li>Fixed a lot of bugs and improved stability</li>
      <li>Updated translations</li>
    </ul>"#
}
