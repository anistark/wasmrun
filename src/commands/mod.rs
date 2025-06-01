mod clean;
mod compile;
mod init;
mod run;
mod stop;
mod verify;

pub use clean::handle_clean_command;
pub use compile::handle_compile_command;
// pub use init::handle_init_command;
pub use run::handle_run_command;
pub use stop::handle_stop_command;
pub use verify::{handle_inspect_command, handle_verify_command, verify_wasm, VerificationResult};
