#!/bin/sh

app_id="com.stremio.Stremio.Devel"
cwd="flatpak"

python3 $cwd/flatpak-builder-tools/cargo/flatpak-cargo-generator.py Cargo.lock -o $cwd/cargo-sources.json

server_version=$(sed -n '/package.metadata.server/,$p' Cargo.toml | grep -m 1 'version =' | awk -F'"' '{print $2}')
server_file_url=https://dl.strem.io/server/$server_version/desktop/server.js
sha256_checksum=$(curl -fsSL $server_file_url | sha256sum | awk '{print $1}')
cat > $cwd/server-source.json <<EOF
{
  "type": "file",
  "url": "$server_file_url",
  "sha256": "$sha256_checksum"
}
EOF

flatpak-builder --repo=$cwd/repo --force-clean $cwd/build $cwd/$app_id.json
flatpak build-bundle $cwd/repo $cwd/$app_id.flatpak $app_id