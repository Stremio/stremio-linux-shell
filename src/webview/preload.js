const createIpc = () => {
    let listeners = [];

    globalThis.IPC_SENDER = (data) => {
        listeners.forEach((listener) => {
            listener({ data });
        });
    };

    const postMessage = (data) => {
        globalThis.IPC_RECEIVER(data);
    };

    const addEventListener = (name, listener) => {
        if (name !== 'message')
            throw Error('Unsupported event');

        listeners.push(listener);
    };

    const removeEventListener = (name, listener) => {
        if (name !== 'message')
            throw Error('Unsupported event');

        listeners = listeners.filter((it) => it !== listener);
    };

    // Clipboard resolver queue for async IPC responses
    let clipboardResolvers = [];

    // Handler for receiving clipboard responses from native shell
    globalThis.CLIPBOARD_RESPONSE = (text) => {
        const resolver = clipboardResolvers.shift();
        if (resolver) {
            resolver(text);
        }
    };

    // IPC request to read clipboard
    const readClipboard = () => {
        return new Promise((resolve) => {
            clipboardResolvers.push(resolve);
            postMessage(JSON.stringify({
                id: Date.now(),
                type: 6,
                args: ['read-clipboard']
            }));
        });
    };

    return {
        postMessage,
        addEventListener,
        removeEventListener,
        readClipboard,
    };
};

window.ipc = createIpc();

// Backward compatibility
window.qt = {
    webChannelTransport: {
        send: window.ipc.postMessage,
    },
};

globalThis.chrome = {
    webview: {
        postMessage: window.ipc.postMessage,
        addEventListener: (name, listener) => {
            window.ipc.addEventListener(name, listener);
        },
        removeEventListener: (name, listener) => {
            window.ipc.removeEventListener(name, listener);
        },
    },
};

window.ipc.addEventListener('message', (message) => {
    window.qt.webChannelTransport.onmessage(message);
});

// Polyfill navigator.clipboard for Wayland support
const originalClipboard = navigator.clipboard;
Object.defineProperty(navigator, 'clipboard', {
    get: () => ({
        readText: async () => {
            try {
                return await window.ipc.readClipboard();
            } catch (e) {
                console.error('Native clipboard read failed, falling back:', e);
                return originalClipboard?.readText?.() || '';
            }
        },
        writeText: async (text) => {
            return originalClipboard?.writeText?.(text);
        },
        read: async () => {
            return originalClipboard?.read?.();
        },
        write: async (data) => {
            return originalClipboard?.write?.(data);
        },
    }),
    configurable: true,
});

console.log('preload');

(function () {
    let lastTitle = "";
    let lastPoster = "";

    let servicesHooked = false;
    let internalMetadata = { title: "", artist: "", poster: "" };

    function hookServices() {
        if (!servicesHooked && window.services && window.services.core) {
            try {
                servicesHooked = true;
                setInterval(async () => {
                    try {
                        if (window.services && window.services.core && window.services.core.transport) {
                            const state = await window.services.core.transport.getState('player');
                            if (state && state.event && state.event.name === 'video-changed') { // Assuming state might contain event info
                                // Clear internal metadata cache
                                internalMetadata = { title: "", artist: "", poster: "" };
                                lastTitle = "";
                                lastPoster = "";
                                // console.log("Video Changed - Cleared Metadata Cache");
                            }
                            if (state && state.metaItem) {
                                // Extract Metadata
                                let seriesName = state.metaItem.name || "";
                                let epTitle = "";
                                let art = "";

                                // 1. Try to find the specific video (Episode)
                                if (state.selected && state.selected.streamRequest && state.selected.streamRequest.path) {
                                    const vidId = state.selected.streamRequest.path.id;
                                    const video = state.metaItem.videos.find(v => v.id === vidId);

                                    if (video) {
                                        // ARTWORK: User wants "Window Preview" (Episode Thumbnail)
                                        if (video.thumbnail) {
                                            art = video.thumbnail;
                                        } else if (video.thumbnailUrl) { // sometimes different prop
                                            art = video.thumbnailUrl;
                                        }

                                        // TITLE: Episode Name
                                        if (video.title) {
                                            // Clean title (remove "Episode 1" redundancy if needed, but usually fine)
                                            epTitle = video.title;

                                            // If video has season/episode info, maybe prepend SxxExx?
                                            // Web Player style: "S1:E1 Episode Title" or just "Episode Title"?
                                            // User said: "eps name on top"
                                            if (video.season && video.episode) {
                                                // epTitle = `${video.season}x${video.episode} - ${video.title}`;
                                                // Actually, modern players just show Episode Title usually.
                                                // But Stremio Web UI usually shows "S1 E1: The Vanishing..."?
                                                // Let's stick to just the Title if it's descriptive, or prepend SxxExx if it's generic.
                                                if (!epTitle.includes(video.season + "x")) {
                                                    epTitle = `${video.season}x${video.episode} ${epTitle}`;
                                                }
                                            }
                                        } else {
                                            // Fallback if no specific title
                                            if (video.season && video.episode) {
                                                epTitle = `${seriesName} (${video.season}x${video.episode})`;
                                            }
                                        }
                                    }
                                }

                                // Fallback Artwork
                                if (!art) {
                                    if (state.metaItem.background) {
                                        art = state.metaItem.background; // Fanart is wide (window preview style)
                                        // } else if (state.metaItem.poster) {
                                        //    art = state.metaItem.poster; // User does not want poster
                                    } else {
                                        art = state.metaItem.logo;
                                    }
                                }

                                // LOGO: Priority is Logo
                                if (state.metaItem.logo) {
                                    internalMetadata.logo = state.metaItem.logo;
                                }

                                internalMetadata.title = epTitle;
                                internalMetadata.artist = seriesName;
                                internalMetadata.art_url = art;

                                console.log("[Preload Debug] Internal Metadata Update:");
                                console.log("  Title:", epTitle);
                                console.log("  Artist (Series):", seriesName);
                                console.log("  Logo:", internalMetadata.logo);
                                console.log("  Poster:", art);
                            }
                        }
                    } catch (err) { }
                }, 2000);
            } catch (e) {
                console.error("Failed to hook services", e);
            }
        }
    }

    function checkMetadata() {
        hookServices();

        let title = document.title;
        let artist = "";
        let poster = ""; // Revert to poster for consistency with rest of function
        let logo = "";

        // Source A: Internal State (Highest Priority for correctness)
        if (internalMetadata.art_url) {
            poster = internalMetadata.art_url;
        }
        if (internalMetadata.title) {
            title = internalMetadata.title;
        }
        if (internalMetadata.artist) {
            artist = internalMetadata.artist;
        }
        if (internalMetadata.logo) {
            logo = internalMetadata.logo;
        }

        // Source 2: MediaSession API (Secondary)
        if (!poster && navigator.mediaSession && navigator.mediaSession.metadata && navigator.mediaSession.metadata.artwork.length > 0) {
            poster = navigator.mediaSession.metadata.artwork[0].src;
        }

        // Source 2: MediaSession for Artist (Reliable for Series Name)
        if (!artist && navigator.mediaSession && navigator.mediaSession.metadata && navigator.mediaSession.metadata.artist) {
            artist = navigator.mediaSession.metadata.artist;
        }

        // Source 3: MediaSession for Logo (Check artwork for "logo" path)
        if (!logo && navigator.mediaSession && navigator.mediaSession.metadata && navigator.mediaSession.metadata.artwork.length > 0) {
            // Iterate to find something that looks like a logo
            for (const art of navigator.mediaSession.metadata.artwork) {
                if (art.src && art.src.includes("/logo/")) {
                    logo = art.src;
                    break;
                }
            }
            // If still no logo, but we found a poster that is actually a logo?
            if (!logo && poster && poster.includes("/logo/")) {
                logo = poster;
            }
        }

        // Source 2: MediaSession for Title (Reliable)
        if (navigator.mediaSession && navigator.mediaSession.metadata && navigator.mediaSession.metadata.title) {
            title = navigator.mediaSession.metadata.title;
        }

        // Source 2: DOM scraping fallback
        if (!title || title.trim().toLowerCase() === "stremio") {
            const titleSelectors = [
                '[class*="nav-bar-layer"]', // From HorizontalNavBar in Player.js
                '.player-title',
                '.video-title',
                '.meta-title',
                '.info-title',
                'div[class*="title"]',
                'h1'
            ];
            for (const selector of titleSelectors) {
                const el = document.querySelector(selector);
                if (el && el.innerText && el.innerText.trim().length > 0) {
                    title = el.innerText.trim();
                    break;
                }
            }
        }

        // Try to find poster in the UI with more robust selectors
        // 1. Player overlay poster
        // 2. Details page poster
        // 3. Generic poster class
        const selectors = [
            '.player-poster img',
            '.meta-poster img',
            'img[class*="poster"]',
            'div[class*="poster"] > img',
            'img[src*="poster"]'
        ];

        for (const selector of selectors) {
            const el = document.querySelector(selector);
            if (el && el.src) {
                poster = el.src;
                break;
            }
        }

        // DOM Fallback for Logo
        if (!logo) {
            const logoSelectors = [
                'img[src*="logo"]',
                '.logo-container img',
                '.logo img'
            ];
            for (const selector of logoSelectors) {
                const el = document.querySelector(selector);
                if (el && el.src) {
                    logo = el.src;
                    break;
                }
            }
        }

        // Sanitize title if it contains generic Stremio text only when we have a better one?
        // No, let Rust handle sanitization. We just send what we see.

        // If title is just "Stremio" but we have a poster, it might be the player loading.
        // But usually document.title updates to the video name.

        if (title !== lastTitle || poster !== lastPoster || logo !== internalMetadata.lastLogo) {
            lastTitle = title;
            lastPoster = poster;
            internalMetadata.lastLogo = logo; // store last logo to prevent spam

            console.log('Sending metadata update:', title, poster, logo);

            window.ipc.postMessage(JSON.stringify({
                id: Date.now(),
                type: 6,
                args: ["metadata-update", { title: title, artist: artist, art_url: poster, logo: logo }]
            }));
        }
    }

    setInterval(checkMetadata, 2000);
})();
