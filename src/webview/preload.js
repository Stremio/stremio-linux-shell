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
