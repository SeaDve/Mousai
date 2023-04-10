<h1 align="center">
  <img src="data/icons/io.github.seadve.Mousai.svg" alt="Mousai" width="192" height="192"/>
  <br>
  Mousai
</h1>

<p align="center">
  <strong>Identify songs in seconds</strong>
</p>

<p align="center">
  <a href="https://flathub.org/apps/details/io.github.seadve.Mousai">
    <img alt="Download on Flathub" src="https://flathub.org/assets/badges/flathub-badge-i-en.svg" width="200"/>
  </a>
  <br>
  <a href="https://liberapay.com/SeaDve/donate">
    <img alt="Donate through Liberapay" src="https://liberapay.com/assets/widgets/donate.svg"/>
  </a>
</p>

<br>

<p align="center">
  <a href="https://hosted.weblate.org/engage/kooha">
    <img alt="Translation status" src="https://hosted.weblate.org/widgets/kooha/-/mousai/svg-badge.svg"/>
  </a>
  <a href="https://github.com/SeaDve/Mousai/actions/workflows/ci.yml">
    <img alt="Build status" src="https://github.com/SeaDve/Mousai/actions/workflows/ci.yml/badge.svg"/>
  </a>
  <a href="https://flathub.org/apps/details/io.github.seadve.Mousai">
    <img alt="Flathub downloads" src="https://img.shields.io/badge/dynamic/json?color=informational&label=downloads&logo=flathub&logoColor=white&query=%24.installs_total&url=https%3A%2F%2Fflathub.org%2Fapi%2Fv2%2Fstats%2Fio.github.seadve.Mousai"/>
  </a>
  <a href="https://repology.org/project/mousai/versions">
    <img alt="Packaging status" src="https://repology.org/badge/tiny-repos/mousai.svg"/>
  </a>
</p>

<br>

<p align="center">
  <img src="data/resources/screenshots/preview.png" alt="Preview"/>
</p>

Discover songs you are aching to know with an easy-to-use interface.

Mousai is a simple application that can identify songs similar to Shazam. Just
click the listen button, and then wait a few seconds. It will magically return
the title and artist of that song!

Note: This uses the API of audd.io, so it is necessary to log in to their site to get more trials.

Why you will love Mousai?
* 🎵 Recognize songs within seconds or save for later when offline
* 🎙️ Use microphone or desktop audio
* 🎸 Build a repertoire of recognized songs
* 🎼 Quickly preview the song within the interface
* 🌐 Browse and listen the song from different providers
* 📱 Easy-to-use user interface

## 🌈 AudD

AudD is a music recognition API that makes Mousai possible. For more information,
you can check their [Privacy Policy](https://audd.io/privacy/) and [Terms of Service](https://audd.io/terms/).


## 🏗️ Building from source

### GNOME Builder
GNOME Builder is the environment used for developing this application. It can use Flatpak manifests to create a consistent building and running environment cross-distro. Thus, it is highly recommended you use it.

1. Download [GNOME Builder](https://flathub.org/apps/details/org.gnome.Builder).
2. In Builder, click the "Clone Repository" button at the bottom, using `https://github.com/SeaDve/Mousai.git` as the URL.
3. Click the build button at the top once the project is loaded.

### Meson
```
git clone https://github.com/SeaDve/Mousai.git
cd Mousai
meson _build --prefix=/usr/local
ninja -C _build install
```


## 🙌 Help translate Mousai

You can help Mousai translate into your native language. If you found any typos
or think you can improve a translation, you can use the [Weblate](https://hosted.weblate.org/engage/kooha/) platform.


## ☕ Support me and the project

Mousai is free and will always be for everyone to use. If you like the project and
would like to support and fund it, you may donate through [Liberapay](https://liberapay.com/SeaDve/).


## 💝 Acknowledgment

I would like to express my gratitude to the [contributors](https://github.com/SeaDve/Mousai/graphs/contributors)
and [translators](https://hosted.weblate.org/engage/kooha/) of the project.

I would also like to thank the open-source software projects, libraries, and APIs that were
used in developing this app, such as GStreamer, GTK, libadwaita, [AudD](https://audd.io/), etc., for making Mousai possible.

I would also like to acknowledge GNOME Shell whose
[issue templates](https://gitlab.gnome.org/GNOME/gnome-shell/-/tree/main/.gitlab/issue_templates)
served as the basis of Mousai's issue templates.
