//If you want to use the macro in any module without the declaration below and then refer to it 
// with use debug_message::print_debug_message, you can use the #[macro_export] attribute
//otherwise you need to export it at the bottom of the ile with: pub(crate) use print_debug_message;
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