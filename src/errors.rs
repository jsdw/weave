pub type Error = Box<dyn std::error::Error + Send + Sync + 'static>;

#[macro_export]
macro_rules! err {
    ($($tt:tt)*) => ({
        let err: Error = format!($($tt)*).into();
        err
    })
}