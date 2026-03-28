//! Agent mode: Lightweight, secure, embeddable code sandbox for AI agents.
//!
//! Provides session-based WASM sandbox management with REST API
//! for LLM agent integration. Each session gets isolated WASI
//! filesystem, environment, and output buffers.

pub mod api;
pub mod server;
pub mod session;
pub mod tools;
