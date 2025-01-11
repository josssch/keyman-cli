use clap::Parser;
use cli::KeyManCli;

mod cli;
mod platform;
mod store;

fn main() {
    let cli = KeyManCli::parse();
    cli.handle();
}
