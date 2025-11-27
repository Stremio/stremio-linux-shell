use cef::{
    Browser, CefString, CefStringUtf16, ImplBrowser, ImplFrame, ImplListValue, ImplProcessMessage,
    process_message_create,
};
use cef_dll_sys::cef_process_id_t;

pub fn send_process_message(
    browser: Option<&mut Browser>,
    name: &str,
    arg: Option<&CefStringUtf16>,
) {
    let name = CefString::from(name);
    let mut message =
        process_message_create(Some(&name)).expect("Failed to create process message");

    if let Some(arg) = arg {
        let arguments = message.argument_list().unwrap();
        arguments.set_string(0, Some(arg));
    }

    if let Some(browser) = browser
        && let Some(main_frame) = browser.main_frame()
    {
        main_frame.send_process_message(cef_process_id_t::PID_BROWSER.into(), Some(&mut message));
    }
}
