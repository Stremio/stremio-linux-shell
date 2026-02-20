#!/bin/sh

app_id="com.stremio.Stremio.Devel"
cwd="flatpak"

python3 $cwd/flatpak-builder-tools/cargo/flatpak-cargo-generator.py Cargo.lock -o $cwd/cargo-sources.json

flatpak-builder --repo=$cwd/repo --force-clean $cwd/build $cwd/$app_id.json
flatpak build-bundle $cwd/repo $cwd/$app_id.flatpak $app_id