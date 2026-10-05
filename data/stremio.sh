#!/bin/bash

# Use GSK OpenGL renderer for Nvidia cards
if ls /dev/nvidia0 &>/dev/null 2>&1; then
    export GSK_RENDERER=opengl
fi

# Ignore host font caches, which can be written in an incompatible format by other programs
if [ -f /app/etc/fonts/stremio.conf ]; then
    export FONTCONFIG_FILE=/app/etc/fonts/stremio.conf
fi

exec /app/libexec/stremio/stremio "$@"