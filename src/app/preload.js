const createIpc = () => {
    let listeners = [];

    globalThis.__postMessage = (data) => {
        listeners.forEach((listener) => {
            listener({ data });
        });
    };

    const postMessage = (data) => {
        window.webkit.messageHandlers.ipc.postMessage(data);
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

    return {
        postMessage,
        addEventListener,
        removeEventListener,
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

console.log('preload script injected');
