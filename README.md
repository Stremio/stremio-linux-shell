<div align="center">

![Stremio icon](data/icons/com.stremio.Stremio.svg "Stremio icon")

# Stremio on Linux 
Client for Stremio on Linux using [`gtk4`](https://docs.gtk.org/gtk4/) + [`libadwaita`](https://gnome.pages.gitlab.gnome.org/libadwaita/doc/1.8/) + [`WebKitGTK`](https://webkitgtk.org/) + [`libmpv`](https://github.com/mpv-player/mpv/blob/master/DOCS/man/libmpv.rst)

<img src="data/screenshots/screenshot1.png" alrt="Screenshot" width="800" />

</div>

## Installation

```bash
flatpak install com.stremio.Stremio
```

## Development

```bash
git clone --recurse-submodules https://github.com/Stremio/stremio-linux-shell
```

#### Fedora
```bash
dnf install gtk4-devel libadwaita-devel webkitgtk6.0-devel mpv-devel libepoxy-devel flatpak-builder
```

```bash
cargo run --release # RUST_LOG=debug to print debug logs
```

#### Ubuntu
```bash
apt install build-essential pkg-config libgtk-4-dev libadwaita-1-dev libwebkitgtk-6.0-dev libmpv-dev gettext nodejs flatpak-builder
```

```bash
cargo run --release # RUST_LOG=debug to print debug logs
```

#### Flatpak
```bash
python3 -m pip install aiohttp toml
flatpak install -y \
    org.gnome.Sdk//50 \
    org.gnome.Platform//50 \
    org.freedesktop.Sdk.Extension.rust-stable//25.08 \
    org.freedesktop.Platform.codecs-extra//25.08-extra \
    org.freedesktop.Platform.VAAPI.Intel//25.08
```

```bash
./flatpak/build.sh
flatpak install ./flatpak/com.stremio.Stremio.Devel.flatpak
flatpak run com.stremio.Stremio.Devel
```