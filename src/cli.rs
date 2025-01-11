use std::path::PathBuf;

use clap::{Parser, Subcommand};

use crate::store::SshKeyStorage;

#[derive(Debug, Clone, Parser)]
#[command(
    name = "keyman",
    bin_name = "keyman",
    about = "SSH Key Manager for easily swapping your SSH keys around"
)]
pub struct KeyManCli {
    #[clap(subcommand)]
    pub subcommand: Option<Command>,

    #[arg(short, long)]
    pub list: bool,
}

#[derive(Debug, Clone, Subcommand)]
pub enum Command {
    #[clap(name = "add")]
    AddKey {
        private_key: PathBuf,

        #[arg(
            short,
            long,
            help = "A name to identify the key by, default will be the file name"
        )]
        name: Option<String>,
    },

    #[clap(name = "remove", alias = "rm")]
    RemoveKey { name: String },

    #[clap(name = "rename", alias = "mv")]
    RenameKey { name: String, new_name: String },

    #[clap(name = "list", alias = "ls")]
    ListKeys, // todo: options for output formatting

    #[clap(name = "use", alias = "swap")]
    UseKey { key_name: String },

    #[clap(name = "info", alias = "show")]
    InfoKey { name: String },
}

impl KeyManCli {
    pub fn handle(&self) {
        let subcommand = match self.subcommand.as_ref() {
            Some(subcommand) => subcommand,
            // doing this allows for more option in what is considered familiar syntax
            None if self.list => &Command::ListKeys,
            None => {
                // todo: print help
                return;
            }
        };

        let mut store = SshKeyStorage::from_default_file().unwrap_or_default();

        match subcommand {
            Command::ListKeys => {
                let current_key_name = store.get_active_key().map(|k| k.name.as_str());

                for (i, &key) in store.get_keys().iter().enumerate() {
                    let in_use = Some(key.name.as_str()) == current_key_name;

                    println!(
                        "{}. {}{}",
                        i + 1,
                        key.name,
                        if in_use { " (using)" } else { "" }
                    );
                }
            }

            Command::AddKey { private_key, name } => {
                match store.add_key(private_key.clone(), name.as_deref()) {
                    Some(key) => {
                        println!("Added key: {}", key.name);
                        store.save().unwrap();
                    }

                    None => {
                        eprintln!("Failed to add key");
                    }
                }
            }

            Command::UseKey { key_name } => match store.use_key(Some(key_name)) {
                Ok(Some(key)) => {
                    println!("Now using key: {}", key.name);
                    store.save().unwrap();
                }

                Ok(None) => {
                    println!("Key not found: {}", key_name);
                }

                Err(e) => {
                    eprintln!("Failed to use key: {}", e);
                }
            },

            Command::RemoveKey { name } => match store.remove_key(name) {
                Some(key) => {
                    println!("Removed key: {}", key.name);
                    store.save().unwrap();
                }

                None => {
                    println!("Key not found: {}", name);
                }
            },

            Command::RenameKey { name, new_name } => todo!(),
            Command::InfoKey { name } => todo!(),
        }
    }
}
