pub mod account_pool;
pub mod client;
pub mod download;
pub mod logger;
pub mod mail_receiver;
pub mod model;
pub mod search;
pub mod solver;

#[macro_export]
macro_rules! log_info {
    ($($arg:tt)*) => {
        $crate::logger::log(&format!($($arg)*))
    };
}