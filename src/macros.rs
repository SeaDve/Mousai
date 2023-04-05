#[macro_export]
macro_rules! derived_properties {
    () => {
        fn properties() -> &'static [glib::ParamSpec] {
            Self::derived_properties()
        }

        fn set_property(&self, id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
            self.derived_set_property(id, value, pspec);
        }

        fn property(&self, id: usize, pspec: &glib::ParamSpec) -> glib::Value {
            self.derived_property(id, pspec)
        }
    };
}

// FIXME these macros may compute expensive expressions on release builds

/// Like `assert_eq` but logs as error instead of panicking on release builds.
///
/// This should only be used on programmer errors.
#[macro_export]
macro_rules! debug_assert_eq_or_log {
    ($left:expr, $right:expr) => {
        if cfg!(debug_assertions) {
            assert_eq!($left, $right);
        } else {
            match (&$left, &$right) {
                (left_val, right_val) => {
                    if !(*left_val == *right_val) {
                        tracing::error!(
                            r#"assertion failed: `(left == right)`
  left: `{:?}`,
 right: `{:?}`"#,
                            left_val,
                            right_val,
                        );
                    }
                }
            }
        }
    };
}
