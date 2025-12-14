mod browser_process_handler;
mod client;
mod render_process_handler;
mod utils;
mod v8_handler;

use browser_process_handler::WebViewBrowserProcessHandler;
use render_process_handler::WebViewRenderProcessHandler;

use crate::{cef_impl, constants::CMD_SWITCHES};

cef_impl!(
    prefix = "WebView",
    name = App,
    sys_type = cef_dll_sys::cef_app_t,
    {
        fn on_before_command_line_processing(
            &self,
            _process_type: Option<&CefString>,
            command_line: Option<&mut CommandLine>,
        ) {
            if let Some(line) = command_line {
                CMD_SWITCHES.iter().for_each(|switch| {
                    line.append_switch(Some(&CefString::from(switch.to_owned())));
                });

                line.append_switch_with_value(
                    Some(&CefString::from("renderer-process-limit")),
                    Some(&CefString::from("1")),
                );
                line.append_switch_with_value(
                    Some(&CefString::from("max-active-webgl-contexts")),
                    Some(&CefString::from("1")),
                );
                line.append_switch(Some(&CefString::from("disable-site-isolation-trials")));
                line.append_switch(Some(&CefString::from("disable-extensions")));
                line.append_switch(Some(&CefString::from("no-zygote")));
                line.append_switch(Some(&CefString::from("no-proxy-server")));
                line.append_switch(Some(&CefString::from("ignore-certificate-errors")));
                line.append_switch(Some(&CefString::from("disable-web-security")));
                line.append_switch(Some(&CefString::from("allow-running-insecure-content")));

                // High-Performance GPU Flags
                line.append_switch(Some(&CefString::from("enable-gpu-rasterization")));
                line.append_switch(Some(&CefString::from("enable-zero-copy")));
                line.append_switch(Some(&CefString::from("ignore-gpu-blocklist")));
                line.append_switch(Some(&CefString::from("disable-gpu-driver-bug-workarounds")));

                use crate::shared::types::SCALE_FACTOR;
                let scale_factor = SCALE_FACTOR.load(std::sync::atomic::Ordering::Relaxed);
                let scale = f64::from_bits(scale_factor);

                if scale > 0.1 {
                    let switch = CefString::from("force-device-scale-factor");
                    let value = CefString::from(scale.to_string().as_str());
                    line.append_switch_with_value(Some(&switch), Some(&value));
                }
            }
        }

        fn browser_process_handler(&self) -> Option<BrowserProcessHandler> {
            Some(WebViewBrowserProcessHandler::new())
        }

        fn render_process_handler(&self) -> Option<RenderProcessHandler> {
            Some(WebViewRenderProcessHandler::new())
        }
    }
);
