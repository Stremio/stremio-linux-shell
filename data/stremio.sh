#!/bin/bash

export LC_NUMERIC=C
export ANV_DEBUG=video-decode,video-encode
export SERVER_PATH=/usr/libexec/stremio/server.js

# Use GSK OpenGL renderer for Nvidia cards
if ls /dev/nvidia0 &>/dev/null 2>&1; then
    export GSK_RENDERER=opengl
fi

exec /usr/libexec/stremio/stremio "$@"