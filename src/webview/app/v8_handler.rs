use crate::{
    cef_impl,
    webview::constants::{IPC_MESSAGE, IPC_RECEIVER},
};

use super::utils;

cef_impl!(
    prefix = "WebView",
    name = V8Handler,
    sys_type = cef_dll_sys::cef_v8_handler_t,
    {
        fn execute(
            &self,
            name: Option<&CefString>,
            _object: Option<&mut impl ImplV8Value>,
            arguments: Option<&[Option<impl ImplV8Value>]>,
            _retval: Option<&mut Option<impl ImplV8Value>>,
            _exception: Option<&mut CefString>,
        ) -> ::std::os::raw::c_int {
            if is_handler(name, IPC_RECEIVER) {
                if let Some(data) = handler_data(arguments) {
                    send_ipc_message(data);

                    return 1;
                }
            }

            0
        }
    }
);

fn is_handler(name: Option<&CefString>, value: &str) -> bool {
    name.is_some_and(|name| {
        let handler_name = CefString::from(value);
        name.as_slice() == handler_name.as_slice()
    })
}

fn handler_data(arguments: Option<&[Option<impl ImplV8Value>]>) -> Option<CefStringUtf16> {
    arguments.and_then(|arguments| {
        arguments.first().and_then(|value| {
            value
                .as_ref()
                .map(|value| value.get_string_value())
                .map(|value| CefString::from(&value))
        })
    })
}

fn send_ipc_message(data: CefStringUtf16) {
    if let Some(context) = v8_context_get_current_context() {
        if let Some(mut browser) = context.get_browser() {
            utils::send_process_message(Some(&mut browser), IPC_MESSAGE, Some(&data));
        }
    }
}
