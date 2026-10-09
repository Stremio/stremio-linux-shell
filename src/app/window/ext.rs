use ashpd::WindowIdentifier;
use gtk::prelude::*;
use gtk::subclass::prelude::*;

pub trait ASHPDExt {
    fn window_identifier(&self) -> impl Future<Output = Option<WindowIdentifier>> + 'static;
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
}
