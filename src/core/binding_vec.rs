use gtk::glib;

use std::cell::RefCell;

/// A list of [`glib::Binding`]s that are automatically unbound
/// on drop or manually through `unbind_all`.
#[derive(Debug, Default)]
pub struct BindingVec {
    inner: RefCell<Vec<glib::Binding>>,
}

impl BindingVec {
    pub fn push(&self, binding: glib::Binding) {
        self.inner.borrow_mut().push(binding);
    }

    pub fn unbind_all(&self) {
        for binding in self.inner.borrow_mut().drain(..) {
            binding.unbind();
        }
    }
}

impl Drop for BindingVec {
    fn drop(&mut self) {
        self.unbind_all();
    }
}
