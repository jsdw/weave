pub type Error = Box<dyn std::error::Error + 'static>;

#[macro_export]
macro_rules! err {
    ($($tt:tt)*) => ({
        Box::<std::error::Error>::from(format!($($tt)*))
    })
}