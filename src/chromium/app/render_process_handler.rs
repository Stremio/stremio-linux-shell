use cef::{rc::*, *};

use crate::chromium::{
    app::process,
    config::{IPC_MESSAGE, IPC_RECEIVER, READY_MESSAGE},
};

wrap_render_process_handler! {
    pub struct ChromiumRenderProcessHandler;

    impl RenderProcessHandler {
        fn on_browser_created(
            &self,
            browser: Option<&mut Browser>,
            _extra_info: Option<&mut DictionaryValue>,
        ) {
            process::send_message(browser, READY_MESSAGE, None);
        }

        fn on_context_created(
            &self,
            _browser: Option<&mut Browser>,
            _frame: Option<&mut Frame>,
            context: Option<&mut V8Context>,
        ) {
            let name = CefString::from(IPC_RECEIVER);
            let mut handler = ChromiumV8Handler::new();

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
}

wrap_v8_handler! {
    struct ChromiumV8Handler;

    impl V8Handler {
        fn execute(
            &self,
            name: Option<&CefString>,
            _object: Option<&mut V8Value>,
            arguments: Option<&[Option<V8Value>]>,
            _retval: Option<&mut Option<V8Value>>,
            _exception: Option<&mut CefString>,
        ) -> i32 {
            if is_handler(name, IPC_RECEIVER)
                && let Some(data) = handler_data(arguments)
            {
                send_ipc_message(data);

                return 1;
            }

            0
        }
    }
}

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
                .map(|value| value.string_value())
                .map(|value| CefString::from(&value))
        })
    })
}

fn send_ipc_message(data: CefStringUtf16) {
    if let Some(context) = v8_context_get_current_context()
        && let Some(mut browser) = context.browser()
    {
        process::send_message(Some(&mut browser), IPC_MESSAGE, Some(&data));
    }
}
