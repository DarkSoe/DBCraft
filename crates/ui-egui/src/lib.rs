#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unimplemented,
    clippy::todo,
    clippy::unreachable
)]

mod app;
mod grid_view;
mod icons;
mod mysql_charset;
mod table_design;
mod theme;
mod user_view;
mod workspace;

pub use app::{DbCraftApp, run_native};
