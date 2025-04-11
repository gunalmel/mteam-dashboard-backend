// If you want the macro to be usable anywhere without explicitly importing it, use #[macro_export].
// This makes it globally available throughout the crate (and from other crates if it's a library).
// Without #[macro_export], you must explicitly export and import the macro, e.g.:
// pub(crate) use print_debug_message; in lib.rs or mod.rs
// and then use crate::print_debug_message; wherever you want to use it.

#[macro_export]
macro_rules! print_debug_message {
    ($($arg:tt)*) => {
        #[cfg(debug_assertions)]
        {
            println!($($arg)*);
        }
    };
}

pub use print_debug_message;
