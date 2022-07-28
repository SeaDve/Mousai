use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

#[derive(Debug, Default)]
pub struct Cancelled(Option<String>);

impl Cancelled {
    pub fn new(message: &str) -> Self {
        Self(Some(message.to_string()))
    }
}

impl std::fmt::Display for Cancelled {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(ref message) = self.0 {
            f.write_str(message)
        } else {
            f.write_str("Operation was cancelled")
        }
    }
}

impl std::error::Error for Cancelled {}

type CancelledCallback = Box<dyn FnOnce(&Cancellable) + 'static>;

#[derive(Default, Clone)]
pub struct Cancellable(Rc<CancellableInner>);

#[derive(Default)]
struct CancellableInner {
    callbacks: RefCell<Vec<CancelledCallback>>,
    is_cancelled: Cell<bool>,
}

impl std::fmt::Debug for Cancellable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Cancellable")
            .field("is_cancelled", &self.is_cancelled())
            .finish()
    }
}

impl Cancellable {
    pub fn cancel(&self) {
        if self.is_cancelled() {
            log::warn!("Trying to cancel a cancelled cancellable");
            return;
        }

        self.0.is_cancelled.set(true);

        for callback in self.0.callbacks.take().into_iter() {
            callback(self);
        }
    }

    pub fn is_cancelled(&self) -> bool {
        self.0.is_cancelled.get()
    }

    pub fn connect_cancelled(&self, callback: impl FnOnce(&Self) + 'static) {
        self.0.callbacks.borrow_mut().push(Box::new(callback));
    }
}
