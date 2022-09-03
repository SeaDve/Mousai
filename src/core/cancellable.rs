use std::{
    cell::{Cell, RefCell},
    fmt,
};

#[derive(Debug, Default)]
pub struct Cancelled(Option<String>);

impl Cancelled {
    pub fn new(message: &str) -> Self {
        Self(Some(message.to_string()))
    }
}

impl fmt::Display for Cancelled {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(ref message) = self.0 {
            f.write_str(message)
        } else {
            f.write_str("Operation was cancelled")
        }
    }
}

impl std::error::Error for Cancelled {}

type CancelledCallback = Box<dyn FnOnce(&Cancellable) + 'static>;

/// Single-threaded cancellable.
#[derive(Default)]
pub struct Cancellable {
    callbacks: RefCell<Vec<CancelledCallback>>,
    is_cancelled: Cell<bool>,
}

impl fmt::Debug for Cancellable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Cancellable")
            .field("is_cancelled", &self.is_cancelled())
            .finish()
    }
}

impl Cancellable {
    /// Cancel and trigger all the callbacks. Cancelling again
    /// is a no-op.
    pub fn cancel(&self) {
        if self.is_cancelled() {
            tracing::warn!("Trying to cancel a cancelled cancellable");
            return;
        }

        self.is_cancelled.set(true);

        for callback in self.callbacks.take().into_iter() {
            callback(self);
        }
    }

    /// Returns true if the cancellable has been cancelled.
    pub fn is_cancelled(&self) -> bool {
        self.is_cancelled.get()
    }

    /// Register a callback to be called when the cancellable is
    /// cancelled. This would be called only once, even if the
    /// cancellable is cancelled again.
    ///
    /// If the cancellable is already cancelled, the callback will
    /// be called immediately.
    pub fn connect_cancelled(&self, callback: impl FnOnce(&Self) + 'static) {
        if self.is_cancelled() {
            callback(self);
            return;
        }

        self.callbacks.borrow_mut().push(Box::new(callback));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::rc::Rc;

    #[test]
    fn cancel() {
        let c = Cancellable::default();
        assert!(!c.is_cancelled());

        c.cancel();
        assert!(c.is_cancelled());
    }

    #[test]
    fn cancel_twice() {
        let c = Cancellable::default();
        assert!(!c.is_cancelled());

        let n_called = Rc::new(Cell::new(0));

        let n_called_clone = Rc::clone(&n_called);
        c.connect_cancelled(move |c| {
            assert!(c.is_cancelled());
            n_called_clone.set(n_called_clone.get() + 1);
        });
        assert_eq!(n_called.get(), 0);

        c.cancel();
        assert_eq!(n_called.get(), 1);

        c.cancel();
        assert_eq!(n_called.get(), 1);
    }

    #[test]
    fn connect() {
        let c = Cancellable::default();
        assert!(!c.is_cancelled());

        let called = Rc::new(Cell::new(false));

        let called_clone = Rc::clone(&called);
        c.connect_cancelled(move |c| {
            assert!(c.is_cancelled());
            called_clone.set(true);
        });
        assert!(!called.get());

        c.cancel();
        assert!(called.get());
    }

    #[test]
    fn connect_after_cancelled() {
        let c = Cancellable::default();
        assert!(!c.is_cancelled());

        c.cancel();
        assert!(c.is_cancelled());

        let called = Rc::new(Cell::new(false));

        let called_clone = Rc::clone(&called);
        c.connect_cancelled(move |c| {
            assert!(c.is_cancelled());
            called_clone.set(true);
        });
        assert!(called.get());

        assert!(c.is_cancelled());
    }
}
