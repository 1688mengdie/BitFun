//! Full copy of `bitfun_core` AI client + stream stack for installer connection tests (no `bitfun_core` dependency).
#![allow(dead_code)]

pub mod client;
pub mod json_checker;
pub mod proxy;
pub mod providers;
pub mod types;

pub use client::AIClient;
