mod app;
mod config;
mod constants;
mod instance;
mod ipc;
mod mpris;
mod player;
mod server;
mod shared;
mod tray;
mod webview;

use app::{App, AppEvent};
use clap::Parser;
use config::Config;
use constants::{STARTUP_URL, URI_SCHEME};
use glutin::{display::GetGlDisplay, surface::GlSurface};
use instance::{Instance, InstanceEvent};
use ipc::{IpcEvent, IpcEventMpv};
use mpris::start_mpris_service;
use player::{Player, PlayerEvent};
use rust_i18n::i18n;
use server::Server;
use shared::{
    types::{MprisCommand, UserEvent},
    with_gl, with_renderer_read, with_renderer_write,
};
use std::{num::NonZeroU32, process::ExitCode, rc::Rc, time::Duration};
use tray::Tray;
use webview::{WebView, WebViewEvent};
use winit::{
    event_loop::{ControlFlow, EventLoop},
    platform::pump_events::{EventLoopExtPumpEvents, PumpStatus},
};

i18n!("locales", fallback = "en");

#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

static GPU_WARNING: once_cell::sync::OnceCell<Option<String>> = once_cell::sync::OnceCell::new();

#[derive(Parser, Debug)]
#[command(version, ignore_errors(true))]
struct Args {
    /// Open dev tools
    #[arg(short, long)]
    dev: bool,
    /// Startup url
    #[arg(short, long, default_value = STARTUP_URL)]
    url: String,
    /// Open a deeplink
    #[arg(short, long)]
    open: Option<String>,
    /// Disable server
    #[arg(short, long)]
    no_server: bool,
}

fn main() -> ExitCode {
    tracing_subscriber::fmt::init();

    let args = Args::parse();
    let config = Config::new();

    let mut event_loop = EventLoop::<UserEvent>::with_user_event()
        .build()
        .expect("Failed to create event loop");

    event_loop.set_control_flow(ControlFlow::Wait);

    let event_loop_proxy = event_loop.create_proxy();

    let mut webview = WebView::new(config.webview, event_loop_proxy.clone());
    if webview.should_exit() {
        return ExitCode::SUCCESS;
    }

    let instance = Instance::new(config.instance);
    if instance.running() {
        if let Some(deeplink) = args.open {
            instance.send(deeplink);
        }

        return ExitCode::SUCCESS;
    }

    instance.start();

    let mut server = Server::new(config.server);
    if !args.no_server {
        server.start(args.dev).expect("Failed to start server");
    }

    let tray = Tray::new(config.tray);
    let mut app = App::new();
    let mut player = Player::new(event_loop_proxy.clone());
    let mpris_controller = start_mpris_service(event_loop_proxy.clone());

    let mut needs_redraw = false;

    loop {
        let timeout = match needs_redraw {
            true => Some(Duration::ZERO),
            false => None,
        };

        let status = event_loop.pump_app_events(timeout, &mut app);

        if let PumpStatus::Exit(exit_code) = status {
            server.stop().expect("Failed to stop server");
            webview.stop();
            instance.stop();
            shared::drop_renderer();
            shared::drop_gl();

            break ExitCode::from(exit_code as u8);
        }

        if needs_redraw {
            with_gl(|surface, context| {
                with_renderer_read(|renderer| {
                    renderer.clear_fbo();
                    player.render(renderer.fbo, renderer.width, renderer.height);
                    renderer.draw();
                });

                surface
                    .swap_buffers(context)
                    .expect("Failed to swap buffers");

                player.report_swap();
            });

            needs_redraw = false;
        }

        instance.events(|event| match event {
            InstanceEvent::Open(deeplink) => {
                event_loop_proxy.send_event(UserEvent::Raise).ok();

                if deeplink.starts_with(URI_SCHEME) {
                    let message = ipc::create_response(IpcEvent::OpenMedia(deeplink.to_string()));
                    webview.post_message(message);
                }
            }
        });

        tray.events(|event| {
            event_loop_proxy.send_event(event).ok();
        });

        app.events(|event| match event {
            AppEvent::Init => {
                webview.start();
            }
            AppEvent::Ready => {
                shared::with_gl(|surface, _| {
                    player.setup(Rc::new(surface.display()));
                });
                // Observe properties needed for MPRIS
                player.observe_property("pause".to_string());
                player.observe_property("media-title".to_string());
                player.observe_property("duration".to_string());
                player.observe_property("time-pos".to_string());
                player.observe_property("speed".to_string());
            }
            AppEvent::Resized(size) => {
                with_gl(|surface, context| {
                    surface.resize(
                        context,
                        NonZeroU32::new(size.0 as u32).unwrap(),
                        NonZeroU32::new(size.1 as u32).unwrap(),
                    );

                    with_renderer_write(|renderer| {
                        renderer.resize(size.0, size.1);
                    });

                    webview.update();
                    needs_redraw = true;
                });
            }
            AppEvent::ScaleFactorChanged(scale_factor) => {
                webview.scale_factor_changed(scale_factor);
                needs_redraw = true;
            }
            AppEvent::Focused(state) => {
                webview.focused(state);
            }
            AppEvent::Visibility(visible) => {
                let message = ipc::create_response(IpcEvent::Visibility(visible));
                webview.post_message(message);

                tray.update(visible);

                if visible {
                    shared::with_gl(|surface, _| {
                        player.setup(Rc::new(surface.display()));
                    });
                } else {
                    player.release();
                }
            }
            AppEvent::Minimized(minimized) => {
                let message = ipc::create_response(IpcEvent::Minimized(minimized));
                webview.post_message(message);
            }
            AppEvent::Fullscreen(fullscreen) => {
                let message = ipc::create_response(IpcEvent::Fullscreen(fullscreen));
                webview.post_message(message);
            }
            AppEvent::MouseMoved(state) => {
                webview.mouse_moved(state);
            }
            AppEvent::MouseWheel(state) => {
                webview.mouse_wheel(state);
            }
            AppEvent::MouseInput(state) => {
                webview.mouse_input(state);
            }
            AppEvent::TouchInput(touch) => {
                webview.touch_input(touch);
            }
            AppEvent::KeyboardInput((key_event, modifiers)) => {
                webview.keyboard_input(key_event, modifiers);
            }
            AppEvent::FileHover((path, state)) => {
                webview.file_hover(path, state);
            }
            AppEvent::FileDrop(state) => {
                webview.file_drop(state);
            }
            AppEvent::FileCancel => {
                webview.file_cancel();
            }
            AppEvent::MprisCommand(cmd) => match cmd {
                MprisCommand::Play => {
                    player.set_property(player::MpvProperty(
                        "pause".to_string(),
                        Some(serde_json::Value::Bool(false)),
                    ));
                }
                MprisCommand::Pause => {
                    player.set_property(player::MpvProperty(
                        "pause".to_string(),
                        Some(serde_json::Value::Bool(true)),
                    ));
                }
                MprisCommand::PlayPause => {
                    player.command("cycle".to_string(), vec!["pause".to_string()])
                }
                MprisCommand::Stop => player.command("stop".to_string(), vec![]),
                MprisCommand::Next => {
                    let message = ipc::create_response(IpcEvent::NextVideo);
                    webview.post_message(message);
                }
                MprisCommand::Previous => {
                    let message = ipc::create_response(IpcEvent::PreviousVideo);
                    webview.post_message(message);
                }
                MprisCommand::Seek(offset) => {
                    player.command(
                        "seek".to_string(),
                        vec![
                            (offset as f64 / 1_000_000.0).to_string(),
                            "relative".to_string(),
                        ],
                    );
                }
                MprisCommand::SetPosition(position) => {
                    player.command(
                        "seek".to_string(),
                        vec![
                            (position as f64 / 1_000_000.0).to_string(),
                            "absolute".to_string(),
                        ],
                    );
                }
                MprisCommand::SetRate(rate) => {
                    player.set_property(player::MpvProperty(
                        "speed".to_string(),
                        Some(serde_json::Value::Number(
                            serde_json::Number::from_f64(rate).unwrap(),
                        )),
                    ));
                }
            },
        });

        webview.events(|event| match event {
            WebViewEvent::Ready => {
                webview.navigate(&args.url);
                webview.dev_tools(args.dev);
            }
            WebViewEvent::Loaded => {
                webview.apply_zoom();
                if let Some(deeplink) = &args.open
                    && deeplink.starts_with(URI_SCHEME)
                {
                    let message = ipc::create_response(IpcEvent::OpenMedia(deeplink.to_string()));
                    webview.post_message(message);
                }

                // Check for GPU configuration issues (Moved here to ensure UI is ready to receive IPC)
                let gpu_warning = GPU_WARNING.get_or_init(|| {
                    let mut warning = None;
                    with_renderer_read(|renderer| {
                        let name = renderer.renderer_name.to_lowercase();
                        tracing::info!("Detected Renderer: {}", renderer.renderer_name);

                        let is_igpu = name.contains("intel")
                            || name.contains("llvmpipe")
                            || name.contains("softpipe");

                        if is_igpu {
                            if let Ok(output) = std::process::Command::new("lspci").output() {
                                let stdout = String::from_utf8_lossy(&output.stdout).to_lowercase();
                                let has_dgpu = stdout.lines().any(|line| {
                                    (line.contains("vga") || line.contains("3d"))
                                        && (line.contains("nvidia")
                                            || line.contains("amd")
                                            || line.contains("radeon"))
                                });

                                if has_dgpu {
                                    warning = Some(format!(
                                    "Integrated GPU detected ({}) while dedicated GPU is available. Performance may be degraded.",
                                    renderer.renderer_name
                                ));
                                }
                            }
                        }
                    });
                    warning
                });

                if let Some(msg) = gpu_warning {
                     tray.show_warning(msg.clone());
                }
            }
            WebViewEvent::Paint => {
                needs_redraw = true;
            }
            WebViewEvent::Resized => {
                webview.update();
                needs_redraw = true;
            }
            WebViewEvent::Cursor(cursor) => {
                app.set_cursor(cursor);
            }
            WebViewEvent::Open(url) => {
                futures::executor::block_on(app.open_url(url));
            }
            WebViewEvent::Ipc(data) => ipc::parse_request(data, |event| match event {
                IpcEvent::Init(id) => {
                    let message = ipc::create_response(IpcEvent::Init(id));
                    webview.post_message(message);
                }
                IpcEvent::Fullscreen(state) => {
                    app.set_fullscreen(state);
                }
                IpcEvent::OpenExternal(url) => {
                    futures::executor::block_on(app.open_url(url));
                }
                IpcEvent::AppReady => {
                    // App is ready, no action needed
                }
                IpcEvent::ReadClipboard => {
                    let text = arboard::Clipboard::new()
                        .and_then(|mut clipboard| clipboard.get_text())
                        .unwrap_or_default();
                    webview.send_clipboard_response(text);
                }
                IpcEvent::Quit => {
                    event_loop_proxy.send_event(UserEvent::Quit).ok();
                }
                IpcEvent::Mpv(event) => match event {
                    IpcEventMpv::Observe(name) => {
                        player.observe_property(name.clone());
                        // Immediately send the current value of the observed property
                        if let Ok(value) = player.get_property(&name) {
                             let message = ipc::create_response(IpcEvent::Mpv(IpcEventMpv::Change(
                                player::MpvProperty(name, Some(value)),
                            )));
                            webview.post_message(message);
                        }
                    }
                    IpcEventMpv::Command((name, args)) => {
                        player.command(name, args);
                    }
                    IpcEventMpv::Set(property) => {
                        player.set_property(property);
                    }
                    _ => {}
                },
                _ => {}
            }),
        });

        let mut player_events = Vec::new();
        player.events(|event| player_events.push(event));

        for event in player_events {
            match event {
                PlayerEvent::Start => {
                    futures::executor::block_on(app.disable_idling());
                    // Explicitly unpause on start
                    player.set_property(player::MpvProperty(
                        "pause".to_string(),
                        Some(serde_json::Value::Bool(false)),
                    ));
                    mpris_controller.update_playback_status("Playing");
                }
                PlayerEvent::Stop(error) => {
                    futures::executor::block_on(app.enable_idling());

                    // Explicitly PAUSE the player so MPV reports pause=true (needed for UI state)
                    player.set_property(player::MpvProperty(
                        "pause".to_string(),
                        Some(serde_json::Value::Bool(true)),
                    ));

                    let message = ipc::create_response(IpcEvent::Mpv(IpcEventMpv::Ended(error)));
                    webview.post_message(message);

                    mpris_controller.update_playback_status("Stopped");
                }
                PlayerEvent::Update => {
                    needs_redraw = true;
                }
                PlayerEvent::PropertyChange(property) => {
                    let message =
                        ipc::create_response(IpcEvent::Mpv(IpcEventMpv::Change(property.clone())));
                    webview.post_message(message);

                    match property.name() {
                        "pause" => {
                            if let Ok(value) = property.value() {
                                match value {
                                    player::MpvPropertyValue::Bool(true) => {
                                        mpris_controller.update_playback_status("Paused")
                                    }
                                    player::MpvPropertyValue::Bool(false) => {
                                        mpris_controller.update_playback_status("Playing")
                                    }
                                    _ => {}
                                }
                            }
                        }
                        "media-title" => {
                            if let Ok(player::MpvPropertyValue::String(title)) = property.value() {
                                let clean_title = if title.starts_with("magnet:")
                                    || title.starts_with("http://")
                                    || title.starts_with("https://")
                                    || title.starts_with("file://")
                                {
                                    "Stremio".to_string()
                                } else {
                                    title
                                };
                                mpris_controller.update_metadata(Some(clean_title), None);
                            }
                        }
                        "duration" => {
                            if let Ok(player::MpvPropertyValue::Float(duration)) = property.value()
                            {
                                mpris_controller.update_metadata(None, Some(duration));
                            }
                        }
                        "time-pos" => {
                            if let Ok(player::MpvPropertyValue::Float(position)) = property.value()
                            {
                                mpris_controller.update_position(position);
                            }
                        }
                        "speed" => {
                            // TODO: we don't track rate changes from MPV yet in controller
                        }
                        _ => {}
                    }
                }
                PlayerEvent::MpvError(error) => {
                    let message = ipc::create_response(IpcEvent::Mpv(IpcEventMpv::Error(error)));
                    webview.post_message(message);
                }
            }
        }
    }
}
