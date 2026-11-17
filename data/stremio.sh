#!/bin/bash

# https://github.com/Stremio/stremio-linux-shell/pull/108
export GSK_RENDERER=opengl

# Ignore host font caches, which can be written in an incompatible format by other programs
if [ -f /app/etc/fonts/stremio.conf ]; then
    export FONTCONFIG_FILE=/app/etc/fonts/stremio.conf
fi

exec /app/libexec/stremio/stremio "$@"