#!/bin/sh

# Print a fontconfig configuration equivalent to the runtime one, without the
# host font cache directories added by 50-flatpak.conf.
#
# Flatpak exposes the host caches read-only as /run/host/fonts-cache and
# /run/host/user-fonts-cache. When another program on the host writes caches in
# a newer format under the old file names (e.g. *-le64.cache-9 symlinked to
# *-le64.cache-12), the runtime fontconfig loads them as valid and the WebKit
# web process spins forever matching fonts, leaving the window black.

set -e

sed -n '1,/<include ignore_missing="yes">conf.d<\/include>/p' /etc/fonts/fonts.conf | sed '$d'

for file in /etc/fonts/conf.d/*.conf; do
    if [ "$(basename "$file")" = "50-flatpak.conf" ]; then
        grep -v -e '/run/host/fonts-cache' -e '/run/host/user-fonts-cache' "$file" |
            sed -e '/<?xml/d' -e '/<!DOCTYPE/d' -e '/<\/\{0,1\}fontconfig>/d'
    else
        echo "	<include ignore_missing=\"yes\">$file</include>"
    fi
done

sed -n '/<include ignore_missing="yes">conf.d<\/include>/,$p' /etc/fonts/fonts.conf | sed '1d'
