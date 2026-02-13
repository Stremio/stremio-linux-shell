use cef::sys::{
    cef_permission_request_result_t::CEF_PERMISSION_RESULT_ACCEPT,
    cef_permission_request_types_t::CEF_PERMISSION_TYPE_LOCAL_NETWORK_ACCESS,
    cef_permission_request_types_t::CEF_PERMISSION_TYPE_LOOPBACK_NETWORK,
};
use cef::{rc::*, *};

wrap_permission_handler! {
    pub struct ChromiumPermissionHandler;

    impl PermissionHandler {
        fn on_show_permission_prompt(
            &self,
            _browser: Option<&mut Browser>,
            _prompt_id: u64,
            _requesting_origin: Option<&CefString>,
            requested_permissions: u32,
            callback: Option<&mut PermissionPromptCallback>,
        ) -> i32 {
            if requested_permissions == CEF_PERMISSION_TYPE_LOCAL_NETWORK_ACCESS as u32
                || requested_permissions == CEF_PERMISSION_TYPE_LOOPBACK_NETWORK as u32
            {
                if let Some(callback) = callback {
                    callback.cont(CEF_PERMISSION_RESULT_ACCEPT.into());
                    return true.into();
                }
            }

            Default::default()
        }
    }
}
