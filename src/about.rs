use gettextrs::gettext;
use gtk::{
    glib::{self, IsA},
    prelude::*,
};

use std::{env, path::Path};

use crate::config::{APP_ID, VERSION};

pub fn present_window(transient_for: Option<&impl IsA<gtk::Window>>) {
    let win = adw::AboutWindow::builder()
        .modal(true)
        .application_icon(APP_ID)
        .application_name(gettext("Mousai"))
        .developer_name(gettext("Dave Patrick Caberto"))
        .version(VERSION)
        .copyright(gettext("© 2022 Dave Patrick Caberto"))
        .license_type(gtk::License::Gpl30)
        // Translators: Replace "translator-credits" with your names. Put a comma between.
        .translator_credits(gettext("translator-credits"))
        .issue_url("https://github.com/SeaDve/Mousai/issues")
        .support_url("https://github.com/SeaDve/Mousai/discussions")
        .debug_info(debug_info())
        .debug_info_filename("mousai-debug-info")
        .build();

    win.add_link(
        &gettext("Donate (Liberapay)"),
        "https://liberapay.com/SeaDve",
    );
    win.add_link(
        &gettext("Donate (PayPal)"),
        "https://www.paypal.com/paypalme/davecaberto",
    );

    win.add_link(&gettext("GitHub"), "https://github.com/SeaDve/Mousai");
    win.add_link(
        &gettext("Translate"),
        "https://hosted.weblate.org/projects/kooha/mousai",
    );

    win.set_transient_for(transient_for);
    win.present();
}

fn debug_info() -> String {
    let is_flatpak = Path::new("/.flatpak-info").exists();
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

    format!(
        r#"- {APP_ID} {VERSION}
- Flatpak: {is_flatpak}

- Distribution: {distribution}
- Desktop Session: {desktop_session}
- Display Server: {display_server}

- GTK {gtk_version}
- Libadwaita {adw_version}
- Libsoup {soup_version}
- {gst_version_string}"#
    )
}
