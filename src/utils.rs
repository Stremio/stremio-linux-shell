use std::sync::LazyLock;

#[macro_export]
macro_rules! spawn_local {
    ($body:expr) => {
        glib::MainContext::default().spawn_local($body)
    };
}

pub static IS_DESKTOP_KDE: LazyLock<bool> = LazyLock::new(|| {
    std::env::var("XDG_CURRENT_DESKTOP")
        .ok()
        .is_some_and(|value| value == "KDE")
});
