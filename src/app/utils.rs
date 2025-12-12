use glutin::{
    config::{Config, ConfigTemplateBuilder, GlConfig},
    context::{ContextApi, ContextAttributesBuilder, NotCurrentContext},
    display::GetGlDisplay,
    prelude::GlDisplay,
    surface::{Surface, WindowSurface},
};
use glutin_winit::{DisplayBuilder, GlWindow};
use winit::{
    event_loop::ActiveEventLoop,
    window::{Window, WindowAttributes},
};

fn config_picker(configs: Box<dyn Iterator<Item = Config> + '_>) -> Config {
    configs
        .reduce(|accum, config| {
            if config.num_samples() > accum.num_samples() {
                config
            } else {
                accum
            }
        })
        .expect("Failed to pick a config")
}

pub fn create_window(
    event_loop: &ActiveEventLoop,
    window_attributes: WindowAttributes,
) -> (Option<Window>, Config) {
    let template_builder = ConfigTemplateBuilder::new().prefer_hardware_accelerated(Some(true));

    DisplayBuilder::new()
        .with_window_attributes(Some(window_attributes))
        .build(event_loop, template_builder, config_picker)
        .expect("Failed to build display")
}

pub fn create_surface(config: &Config, window: &Option<Window>) -> Surface<WindowSurface> {
    let surface_attributes = window
        .as_ref()
        .expect("Failed to get window")
        .build_surface_attributes(Default::default())
        .expect("Failed to build surface attributes");

    unsafe {
        config
            .display()
            .create_window_surface(config, &surface_attributes)
            .expect("Failed to create surface")
    }
}

pub fn create_context(config: &Config, api: ContextApi) -> NotCurrentContext {
    let context_attributes = ContextAttributesBuilder::new()
        .with_context_api(api)
        .build(None);

    unsafe {
        config
            .display()
            .create_context(config, &context_attributes)
            .expect("Failed to create shared context")
    }
}
