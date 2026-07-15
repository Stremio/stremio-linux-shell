mod app;
mod config;
mod server;
mod utils;

use std::{env, fs, ptr};

use clap::Parser;
use gtk::glib::{ExitCode, object::ObjectExt};
use tokio::runtime::Runtime;

use crate::{
    app::Application,
    config::{DATA_DIR, GETTEXT_DIR_DEV, GETTEXT_DIR_FLATPAK, GETTEXT_DOMAIN, STARTUP_URL},
    server::Server,
};

#[derive(Parser, Debug)]
#[command(version, ignore_errors(true), allow_hyphen_values(true))]
struct Args {
    /// Open dev tools
    #[arg(short, long)]
    dev: bool,
    /// Startup url
    #[arg(short, long, default_value = STARTUP_URL)]
    url: String,
    /// Disable window decorations
    #[arg(short, long)]
    no_window_decorations: bool,

    #[arg(trailing_var_arg = true)]
    trailing: Vec<String>,
}

fn main() -> ExitCode {
    // GTK 4.22 defaults to the Vulkan GSK renderer. Compositing our OpenGL video
    // GLArea through it forces a per-frame GL->Vulkan copy, which dominates
    // playback CPU even when hardware decoding is active. Prefer the GL renderer,
    // unless a renderer was already chosen — data/stremio.sh sets `opengl` for
    // Nvidia, and `opengl` and `gl` are the same renderer here, so the two compose.
    // Must be set before GTK initializes.
    //
    // The name is `gl`: GTK renamed the new GL renderer in 4.18. `ngl` still
    // resolves to it as a deprecated alias, but warns. Any name GTK does not
    // recognize falls back to the Vulkan renderer this is meant to avoid, so the
    // value is worth keeping in step with `GSK_RENDERER=help`.
    //
    // SAFETY: this runs at the very start of main, before any other threads are
    // spawned, so there is no concurrent access to the environment.
    if env::var_os("GSK_RENDERER").is_none() {
        unsafe { env::set_var("GSK_RENDERER", "gl") };
    }

    tracing_subscriber::fmt::init();

    let data_dir = dirs::data_dir()
        .expect("Failed to get data dir")
        .join(DATA_DIR);

    fs::create_dir_all(&data_dir).expect("Failed to create data directory");

    let gettext_dir = match env::var("FLATPAK_ID") {
        Ok(_) => GETTEXT_DIR_FLATPAK,
        Err(_) => GETTEXT_DIR_DEV,
    };

    gettextrs::bindtextdomain(GETTEXT_DOMAIN, gettext_dir).expect("Failed to bind text domain");
    gettextrs::bind_textdomain_codeset(GETTEXT_DOMAIN, "UTF-8")
        .expect("Failed to set the text domain encoding");
    gettextrs::textdomain(GETTEXT_DOMAIN).expect("Failed to switch text domain");

    let library = unsafe { libloading::os::unix::Library::new("libepoxy.so.0") }
        .expect("Failed to load libepoxy");

    epoxy::load_with(|name| {
        unsafe { library.get::<_>(name.as_bytes()) }
            .map(|symbol| *symbol)
            .unwrap_or(ptr::null())
    });

    let args = Args::parse();

    let mut server = Server::new();
    server.start(args.dev).expect("Failed to start server");

    let app = Application::new();
    app.set_property("dev-mode", args.dev);
    app.set_property("startup-url", args.url);
    app.set_property("decorations", !args.no_window_decorations);

    let runtime = Runtime::new().expect("Failed to create Tokio runtime");
    runtime.block_on(app.run(args.trailing))
}
