use crate::{
    cef_impl,
    webview::{
        app::v8_handler::WebViewV8Handler,
        constants::{IPC_RECEIVER, READY_MESSAGE},
    },
};

use super::utils;

cef_impl!(
    prefix = "WebView",
    name = RenderProcessHandler,
    sys_type = cef_dll_sys::cef_render_process_handler_t,
    {
        fn on_browser_created(
            &self,
            browser: Option<&mut Browser>,
            _extra_info: Option<&mut DictionaryValue>,
        ) {
            utils::send_process_message(browser, READY_MESSAGE, None);
        }

        fn on_context_created(
            &self,
            _browser: Option<&mut Browser>,
            _frame: Option<&mut Frame>,
            context: Option<&mut V8Context>,
        ) {
            let name = CefString::from(IPC_RECEIVER);
            let mut handler = WebViewV8Handler::new();

            let mut value = v8_value_create_function(Some(&name), Some(&mut handler))
                .expect("Failed to create a value for function");

            if let Some(context) = context
                && let Some(global) = context.global()
            {
                global.set_value_bykey(
                    Some(&name),
                    Some(&mut value),
                    V8Propertyattribute::default(),
                );
            }
        }
    }
);
