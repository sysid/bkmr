// src/lib.rs
#![crate_type = "lib"]
#![crate_name = "bkmr"]

extern crate skim;

// Core modules
pub mod application;
pub mod domain;
pub mod infrastructure;

// CLI modules
pub mod app_state;
pub mod cli;
pub mod config;
pub mod util;

#[cfg(test)]
mod tests {}
