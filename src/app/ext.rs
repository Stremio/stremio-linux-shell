use std::path::PathBuf;

use ashpd::WindowIdentifier;
use ashpd::desktop::file_chooser::{FileFilter, OpenFileRequest};
use gtk::gio::File;
use gtk::prelude::*;
use gtk::subclass::prelude::*;

pub trait ASHPDExt {
    fn window_identifier(&self) -> impl Future<Output = Option<WindowIdentifier>> + 'static;
    fn choose_file(&self, filter: FileFilter) -> impl Future<Output = Option<PathBuf>> + 'static;
}

impl<T> ASHPDExt for T
where
    T: ObjectSubclass,
    T::Type: IsA<gtk::Widget>,
{
    fn window_identifier(&self) -> impl Future<Output = Option<WindowIdentifier>> + 'static {
        let weak = self.obj().downgrade();

        async move {
            let widget = weak.upgrade()?;
            let native = widget.native()?;
            WindowIdentifier::from_native(&native).await
        }
    }

    fn choose_file(&self, filter: FileFilter) -> impl Future<Output = Option<PathBuf>> + 'static {
        let identifier = self.window_identifier();

        async move {
            let request = OpenFileRequest::default()
                .identifier(identifier.await)
                .modal(true)
                .multiple(false)
                .filter(filter)
                .send()
                .await;

            request.and_then(|r| r.response()).ok().and_then(|files| {
                files
                    .uris()
                    .first()
                    .and_then(|uri| File::for_uri(uri.as_str()).path())
            })
        }
    }
}
