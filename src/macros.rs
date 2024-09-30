//! General purpose utility macros.
//!

/// Yields a &'static str that is the name of the containing function.
///
/// # Example
/// ```
/// fn demo_fn() {
///   use mu_rust_helpers::function;
///
///   std::println!("This function is called {}", function!());
/// }
/// ```
#[macro_export]
macro_rules! function {
    () => {{
        fn f() {}
        fn type_name_of<T>(_: T) -> &'static str {
            core::any::type_name::<T>()
        }
        let name = type_name_of(f);
        name.strip_suffix("::f").unwrap()
    }};
}
