mod app;
mod config;
mod server;
mod utils;

use std::{env, ptr};

use clap::Parser;
use gettextrs::LocaleCategory;
use gtk::glib::{ExitCode, object::ObjectExt};

use crate::{
    app::Application,
    config::{GETTEXT_DIR_DEV, GETTEXT_DIR_FLATPAK, GETTEXT_DOMAIN},
    server::Server,
};

#[derive(Parser, Debug)]
#[command(version, ignore_errors(true))]
struct Args {
    /// Open dev tools
    #[arg(short, long)]
    dev: bool,
}

#[tokio::main]
async fn main() -> ExitCode {
    tracing_subscriber::fmt::init();

    let gettext_dir = match env::var("FLATPAK_ID") {
        Ok(_) => GETTEXT_DIR_FLATPAK,
        Err(_) => GETTEXT_DIR_DEV,
    };

    gettextrs::setlocale(LocaleCategory::LcAll, "fr");
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
    server.setup().await.expect("Failed to setup server");
    server.start(args.dev).expect("Failed to start server");

    let app = Application::new();
    app.set_property("dev_mode", args.dev);
    app.run()
}
