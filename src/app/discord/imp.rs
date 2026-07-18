use std::{
    cell::RefCell,
    rc::Rc,
    sync::mpsc::{self, Sender},
    thread::JoinHandle,
};

use gtk::glib::{self, subclass::prelude::*};
use tracing::error;

use super::worker::{self, DiscordCommand, RichPresenceClient};
use crate::{app::ipc::event::DiscordActivity, spawn_local};

type StatusCallback = Box<dyn Fn(bool)>;

struct WorkerHandle {
    commands: Sender<DiscordCommand>,
    _join: JoinHandle<()>,
}

#[derive(Default)]
pub struct Discord {
    worker: RefCell<Option<WorkerHandle>>,
    status_callback: Rc<RefCell<Option<StatusCallback>>>,
}

#[glib::object_subclass]
impl ObjectSubclass for Discord {
    const NAME: &'static str = "Discord";
    type Type = super::Discord;
    type ParentType = glib::Object;
}

impl Discord {
    pub fn start(&self) {
        if self.worker.borrow().is_some() {
            return;
        }

        let (commands, receiver) = mpsc::channel::<DiscordCommand>();
        let (status_sender, status_receiver) = flume::unbounded::<bool>();

        let join = std::thread::spawn(move || {
            worker::run(
                receiver,
                move |connected| {
                    status_sender.send(connected).ok();
                },
                RichPresenceClient::new,
            );
        });

        let status_callback = self.status_callback.clone();
        spawn_local!(async move {
            while let Ok(connected) = status_receiver.recv_async().await {
                if let Some(callback) = &*status_callback.borrow() {
                    callback(connected);
                }
            }
        });

        *self.worker.borrow_mut() = Some(WorkerHandle {
            commands,
            _join: join,
        });
    }

    pub fn stop(&self) {
        // Dropping the command sender ends the worker's command loop, which
        // closes the Discord connection and lets the thread terminate on
        // its own without blocking the GTK main thread.
        let handle = self.worker.borrow_mut().take();
        drop(handle);
    }

    pub fn connect(&self) {
        if let Err(e) = self.send_command(DiscordCommand::Connect) {
            error!("Failed to queue Discord connect: {e}");
            self.emit_status(false);
        }
    }

    pub fn disconnect(&self) {
        if let Err(e) = self.send_command(DiscordCommand::Disconnect) {
            error!("Failed to queue Discord disconnect: {e}");
            self.emit_status(false);
        }
    }

    pub fn set_activity(&self, activity: DiscordActivity) {
        if let Err(e) = self.send_command(DiscordCommand::SetActivity(activity)) {
            error!("Failed to queue Discord set activity: {e}");
            self.emit_status(false);
        }
    }

    pub fn clear_activity(&self) {
        if let Err(e) = self.send_command(DiscordCommand::ClearActivity) {
            error!("Failed to queue Discord clear activity: {e}");
            self.emit_status(false);
        }
    }

    pub fn set_status_callback<F: Fn(bool) + 'static>(&self, callback: F) {
        self.status_callback
            .borrow_mut()
            .replace(Box::new(callback));
    }

    fn send_command(&self, command: DiscordCommand) -> Result<(), String> {
        let worker = self.worker.borrow();
        let worker = worker.as_ref().ok_or("Discord service not started")?;

        worker
            .commands
            .send(command)
            .map_err(|e| format!("Discord worker unavailable: {e}"))
    }

    fn emit_status(&self, connected: bool) {
        if let Some(callback) = &*self.status_callback.borrow() {
            callback(connected);
        }
    }
}

impl ObjectImpl for Discord {}

#[cfg(test)]
mod tests {
    use std::{cell::RefCell, rc::Rc};

    use crate::app::ipc::event::DiscordActivity;

    fn activity() -> DiscordActivity {
        DiscordActivity {
            state: "Watching".to_owned(),
            details: "Movie".to_owned(),
            image: None,
            start_timestamp: None,
            end_timestamp: None,
        }
    }

    #[test]
    fn set_activity_with_unavailable_worker_reports_disconnected() {
        let discord = super::super::Discord::default();
        let statuses = Rc::new(RefCell::new(Vec::new()));
        let collected = Rc::clone(&statuses);
        discord.connect_status(move |connected| collected.borrow_mut().push(connected));

        discord.set_activity(activity());

        assert_eq!(*statuses.borrow(), [false]);
    }

    #[test]
    fn clear_activity_with_unavailable_worker_reports_disconnected() {
        let discord = super::super::Discord::default();
        let statuses = Rc::new(RefCell::new(Vec::new()));
        let collected = Rc::clone(&statuses);
        discord.connect_status(move |connected| collected.borrow_mut().push(connected));

        discord.clear_activity();

        assert_eq!(*statuses.borrow(), [false]);
    }
}
