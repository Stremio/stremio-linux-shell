pub const FLOAT_PROPERTIES: &[&str] = &[
    "time-pos",
    "duration",
    "volume",
    "speed",
    "sub-pos",
    "sub-scale",
    "sub-delay",
    "cache-buffering-state",
    "demuxer-cache-time",
    "panscan",
];

pub const BOOL_PROPERTIES: &[&str] = &[
    "pause",
    "buffering",
    "seeking",
    "osc",
    "input-default-bindings",
    "input-vo-keyboard",
    "eof-reached",
    "paused-for-cache",
    "keepaspect",
];

pub const STRING_PROPERTIES: &[&str] = &[
    "path",
    "mpv-version",
    "ffmpeg-version",
    "hwdec",
    "vo",
    "track-list",
    "sub-color",
    "sub-back-color",
    "sub-border-color",
    "sid",
    "aid",
    "vid",
    "mute",
    "metadata",
    "video-params",
    "sub-ass-override",
];
