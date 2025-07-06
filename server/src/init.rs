use std::{any, ops::Deref, sync::OnceLock};

/// This type allows you to have global singletons based on a OnceLock.
/// This means we initialize it once and use it later during the application's
/// runtime. Why? Because the current model (passing the singletons around to
/// every function) is not viable and introduces way more boilerplate. Yes, this
/// struct won't allow you to track the dependency flow anymore. And yes, this
/// implementation may result in a panic but if the program is correctly
/// formulated it is fine. As we have only a couple of singletons, this won't be
/// a problem.
pub struct Init<T> {
    inner: OnceLock<T>,
}

impl<T> Init<T> {
    /// create a new init cell
    pub const fn new() -> Init<T> {
        Init { inner: OnceLock::new() }
    }

    /// initialize the cell with a piece of data. this can only be called once,
    /// multiple calls will result in a panic
    pub fn init(&self, what: T) {
        if let Err(_) = self.inner.set(what) {
            panic!("duplicate initialization of init cell of type '{}'", any::type_name::<T>())
        }
    }

    /// get a reference to the data within the cell. calling this before `init`
    /// will result in a panic
    pub fn get(&self) -> &T {
        let Some(inner) = self.inner.get() else {
            // we print the type for debugging purposes
            panic!("access to init cell of type '{}' before initialization", any::type_name::<T>())
        };

        inner
    }
}

impl<T> Deref for Init<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.get()
    }
}
