#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]

mod bindings;

unsafe impl Send for Rust_CThostFtdcMdApi {}
unsafe impl Sync for Rust_CThostFtdcMdApi {}

unsafe impl Send for Rust_CThostFtdcTraderApi {}
unsafe impl Sync for Rust_CThostFtdcTraderApi {}

pub use bindings::*;
