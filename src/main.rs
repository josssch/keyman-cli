use clap::Parser;
use cli::KeyManCli;

mod cli;
mod error;
mod platform;
mod store;

fn main() {
    let cli = KeyManCli::parse();

    match cli.handle() {
        Ok(_) => (),

        // incase any other error occurs that isn't from a subcommand
        Err(err) => {
            eprintln!("Something went wrong: {}", err);
            std::process::exit(1);
        }
    }
}
