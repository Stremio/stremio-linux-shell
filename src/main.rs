mod app;
mod chromium;
mod config;
mod server;
mod shared;
mod utils;

use std::{env, fs, ptr};

use clap::Parser;
use gtk::glib::{ExitCode, object::ObjectExt};
use tokio::runtime::Runtime;

use crate::{
    app::Application,
    chromium::Chromium,
    config::{DATA_DIR, GETTEXT_DIR_DEV, GETTEXT_DIR_FLATPAK, GETTEXT_DOMAIN, STARTUP_URL},
    server::Server,
};

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
    /// Disable window decorations
    #[arg(short, long)]
    no_window_decorations: bool,
}

fn main() -> ExitCode {
    tracing_subscriber::fmt::init();

    let data_dir = dirs::data_dir()
        .expect("Failed to get data dir")
        .join(DATA_DIR);

    fs::create_dir_all(&data_dir).expect("Failed to create data directory");

    let mut chromium = Chromium::new(&data_dir);
    if let Some(exit_code) = chromium.execute() {
        return ExitCode::from(exit_code as u8);
    }

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

    let runtime = Runtime::new().expect("Failed to create Tokio runtime");

    let mut server = Server::new();
    server.start(args.dev).expect("Failed to start server");

    let app = Application::new();
    app.set_property("dev-mode", args.dev);
    app.set_property("startup-url", args.url);
    app.set_property("open-uri", args.open);
    app.set_property("decorations", !args.no_window_decorations);
    app.set_browser(chromium);

    runtime.block_on(app.run())
}
