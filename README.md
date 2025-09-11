<div align="center">

![Stremio icon](data/icons/com.stremio.Stremio.svg "Stremio icon")

# Stremio on Linux 
Client for Stremio on Linux using [`gtk4`](https://github.com/gtk-rs/gtk4-rs) + [`webkitgtk`](https://gitlab.gnome.org/World/Rust/webkit6-rs) + [`libmpv`](https://github.com/mpv-player/mpv/blob/master/DOCS/man/libmpv.rst)

<img src="data/screenshots/screenshot1.png" alrt="Screenshot" width="800" />

</div>

## Installation

```bash
flatpak remote-add --if-not-exists flathub-beta https://flathub.org/beta-repo/flathub-beta.flatpakrepo
flatpak install flathub-beta com.stremio.Stremio
```

## Development

```bash
git clone --recurse-submodules https://github.com/Stremio/stremio-linux-shell
```

### Building

#### Fedora
```bash
dnf install gtk4-devel libadwaita-devel webkitgtk6.0-devel mpv-devel libepoxy-devel flatpak-builder
```

```bash
cargo build --release
```

#### Ubuntu
```bash
apt install build-essential libssl-dev libnss3 libgtk-4-dev libadwaita-1-dev libwebkitgtk-6.0-dev libmpv-dev libepoxy-dev flatpak-builder
```

```bash
cargo build --release
```

#### Flatpak
```bash
flatpak install -y \
    org.gnome.Sdk//48 \
    org.gnome.Platform//48 \
    org.freedesktop.Sdk.Extension.rust-stable//25.08 \
    org.freedesktop.Platform.ffmpeg-full//24.08 \
    org.freedesktop.Platform.VAAPI.Intel//24.08
python3 -m pip install aiohttp tomlkit
```

```bash
./flatpak/build.sh
```