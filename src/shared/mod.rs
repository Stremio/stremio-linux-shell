mod renderer;
pub mod types;

use std::{
    num::NonZeroU32,
    sync::{Mutex, RwLock},
};

use glutin::{
    context::{NotCurrentContext, PossiblyCurrentContext},
    prelude::{NotCurrentGlContext, PossiblyCurrentGlContext},
    surface::{GlSurface, Surface, SwapInterval, WindowSurface},
};
use renderer::Renderer;
use tracing::warn;

pub static RENDERER: RwLock<Option<Renderer>> = RwLock::new(None);

pub fn create_renderer(default_size: (i32, i32), refresh_rate: u32) {
    if let Ok(mut guard) = RENDERER.write() {
        *guard = Some(Renderer::new(default_size, refresh_rate));
    }
}

pub fn with_renderer_read<T: FnOnce(&Renderer)>(handler: T) {
    if let Ok(renderer) = RENDERER.read()
        && let Some(renderer) = renderer.as_ref()
    {
        handler(renderer)
    }
}

pub fn with_renderer_write<T: FnOnce(&mut Renderer)>(handler: T) {
    if let Ok(mut renderer) = RENDERER.write()
        && let Some(renderer) = renderer.as_mut()
    {
        handler(renderer)
    }
}

pub fn drop_renderer() {
    if let Ok(mut renderer) = RENDERER.write() {
        renderer.take();
    }
}

pub static GL_SURFACE: Mutex<Option<Surface<WindowSurface>>> = Mutex::new(None);
pub static GL_CONTEXT: Mutex<Option<NotCurrentContext>> = Mutex::new(None);

pub fn create_gl(surface: Surface<WindowSurface>, context: NotCurrentContext) {
    let current_context = context
        .make_current(&surface)
        .expect("Failed to make context current");

    let swap_interval = SwapInterval::Wait(NonZeroU32::new(1).unwrap());
    surface
        .set_swap_interval(&current_context, swap_interval)
        .map_err(|e| warn!("Failed to enable VSync: {e}"))
        .ok();

    let not_current_context = current_context
        .make_not_current()
        .expect("Failed to make context not current");

    if let Ok(mut guard) = GL_SURFACE.lock() {
        *guard = Some(surface);
    }

    if let Ok(mut guard) = GL_CONTEXT.lock() {
        *guard = Some(not_current_context);
    }
}

pub fn with_gl<T: FnMut(&Surface<WindowSurface>, &PossiblyCurrentContext)>(mut handler: T) {
    if let Ok(surface) = GL_SURFACE.lock()
        && let Some(surface) = surface.as_ref()
        && let Ok(mut guard) = GL_CONTEXT.lock()
        && let Some(context) = guard.take()
    {
        let current_context = context
            .make_current(surface)
            .expect("Failed to make context current");

        handler(surface, &current_context);

        let not_current_context = current_context
            .make_not_current()
            .expect("Failed to make context not current");

        *guard = Some(not_current_context);
    };
}

pub fn drop_gl() {
    if let Ok(mut surface) = GL_SURFACE.lock() {
        surface.take();
    }

    if let Ok(mut context) = GL_CONTEXT.lock() {
        context.take();
    }
}
