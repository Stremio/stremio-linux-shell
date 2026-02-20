use cef::{
    Browser, CefString, CefStringUtf16, ImplBrowser, ImplFrame, ImplListValue, ImplProcessMessage,
    sys::cef_process_id_t::PID_BROWSER,
};

pub fn send_message(browser: Option<&mut Browser>, name: &str, arg: Option<&CefStringUtf16>) {
    let name = CefString::from(name);
    let mut message =
        cef::process_message_create(Some(&name)).expect("Failed to create process message");

    if let Some(arg) = arg {
        let arguments = message.argument_list().unwrap();
        arguments.set_string(0, Some(arg));
    }

    if let Some(browser) = browser
        && let Some(main_frame) = browser.main_frame()
    {
        main_frame.send_process_message(PID_BROWSER.into(), Some(&mut message));
    }
}
