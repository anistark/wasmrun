mod cli;
mod server;
mod template;
mod utils;

fn main() {
    let args = cli::get_args();
    server::run_server(&args.path, args.port);
}
