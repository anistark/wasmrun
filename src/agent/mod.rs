//! Agent mode: Lightweight, secure, embeddable code sandbox for AI agents.
//!
//! Provides session-based WASM sandbox management with REST API
//! for LLM agent integration. Each session gets isolated WASI
//! filesystem, environment, and output buffers.

pub mod api;
pub mod auth;
pub mod executor;
pub mod limits;
pub mod metrics;
pub mod server;
pub mod session;
pub mod shell;
pub mod tools;
