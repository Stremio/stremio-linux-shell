use crate::{
    cef_impl,
    shared::with_renderer_read,
    webview::{BROWSER, app::client::WebViewClient},
};

cef_impl!(
    prefix = "WebView",
    name = BrowserProcessHandler,
    sys_type = cef_dll_sys::cef_browser_process_handler_t,
    {
        fn on_context_initialized(&self) {
            with_renderer_read(|renderer| {
                let mut client = WebViewClient::new();

                let url = CefString::from("about:blank");

                let window_info = WindowInfo {
                    windowless_rendering_enabled: 1,
                    ..Default::default()
                };

                let settings = BrowserSettings {
                    windowless_frame_rate: renderer.refresh_rate as i32,
                    ..Default::default()
                };

                BROWSER.get_or_init(|| {
                    browser_host_create_browser_sync(
                        Some(&window_info),
                        Some(&mut client),
                        Some(&url),
                        Some(&settings),
                        Option::<&mut DictionaryValue>::None,
                        Option::<&mut RequestContext>::None,
                    )
                    .expect("Failed to create browser sync")
                });
            });
        }

        fn on_before_child_process_launch(&self, command_line: Option<&mut CommandLine>) {
            if let Some(line) = command_line {
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
    }
);
