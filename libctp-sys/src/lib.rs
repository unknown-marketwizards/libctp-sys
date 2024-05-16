#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]

#[cfg(target_os = "linux")]
mod bindings_linux;

#[cfg(target_os = "linux")]
pub use bindings_linux::*;

#[cfg(target_os = "windows")]
mod bindings;

#[cfg(target_os = "windows")]
pub use bindings::*;

unsafe impl Send for Rust_CThostFtdcMdApi {}
unsafe impl Sync for Rust_CThostFtdcMdApi {}

unsafe impl Send for Rust_CThostFtdcMdSpi {}
unsafe impl Sync for Rust_CThostFtdcMdSpi {}

unsafe impl Send for Rust_CThostFtdcTraderApi {}
unsafe impl Sync for Rust_CThostFtdcTraderApi {}

unsafe impl Send for Rust_CThostFtdcTraderSpi {}
unsafe impl Sync for Rust_CThostFtdcTraderSpi {}
