use std::path::PathBuf;

use clap::{CommandFactory, Parser, Subcommand};

use crate::store::SshKeyStorage;

pub const BIN_NAME: &str = env!("CARGO_BIN_NAME");

#[derive(Debug, Clone, Parser)]
#[command(
    name = "keyman",
    bin_name = BIN_NAME,
    about = "SSH Key Manager for easily swapping your SSH keys around"
)]
pub struct KeyManCli {
    #[command(subcommand)]
    pub subcommand: Option<Command>,
}

#[derive(Debug, Clone, Subcommand)]
pub enum Command {
    #[command(
        name = "add",
        alias = "new",
        about = "Add a new SSH key with an existing private key",
        arg_required_else_help = true
    )]
    Add {
        #[arg(
            help = "Path to your private key file",
            value_name = "PRIVATE_KEY_PATH"
        )]
        private_key: PathBuf,

        #[arg(
            short,
            long,
            alias = "save-as",
            help = "A name to identify the key by, default will be the file name"
        )]
        name: Option<String>,

        #[arg(
            short,
            long,
            help = "Immediately place this key in use after adding it"
        )]
        use_key: bool,
    },

    #[command(
        name = "use",
        alias = "swap",
        about = "Symlinks the related private/public key files into ~/.ssh folder"
    )]
    Use { key_name: String },

    #[command(
        name = "info",
        alias = "show",
        about = "Show information about a key or the currently active key"
    )]
    Info { key_name: Option<String> },

    #[command(name = "rename", alias = "mv", about = "Rename a key to a new name")]
    Rename { key_name: String, new_name: String },

    #[command(name = "remove", alias = "rm", about = "Remove a key by name")]
    Remove {
        key_name: String,

        #[arg(
            short,
            long,
            help = "Force remove the key without confirmation when it is in use"
        )]
        force: bool,
    },

    #[command(name = "list", aliases = ["ls", "-l", "--list"], about = "List all keys")]
    List, // todo: options for output formatting
}

impl KeyManCli {
    pub fn usage_msg_from(&self, args: &[&str]) -> String {
        format!("{BIN_NAME} {}", args.join(" "))
    }

    pub fn key_typo_msg_from(&self, key_name: &str) -> String {
        format!(
            "No key called '{key_name}' was found, typo? Use `{}` to see your keys.",
            self.usage_msg_from(&["list"])
        )
    }

    pub fn handle(&self) {
        let mut command = Self::command();

        // parse the subcommand, otherwise print help
        // (I know clap provides a way to do this with macros, but
        // this gives more control in the future)
        let subcommand = match self.subcommand {
            Some(ref subcommand) => subcommand,
            None => {
                command.print_help().expect("failed to print help");
                return;
            }
        };

        // create the file or load the existing one
        let mut store = SshKeyStorage::from_default_file().unwrap_or_default();

        match subcommand {
            Command::List => {
                let current_key_name = store.get_active_key().map(|k| k.name.as_str());

                println!("Your SSH keys:");

                for &key in store.get_keys().iter() {
                    let in_use = Some(key.name.as_str()) == current_key_name;

                    println!("  - {}{}", key.name, if in_use { " (in use)" } else { "" });
                }
            }

            Command::Add {
                private_key,
                name,
                use_key,
            } => {
                let new_key_name = match store.add_key(private_key.clone(), name.as_deref()) {
                    Ok(key) => {
                        println!(
                            "Added key '{}' to list of keys, use it with `{}`",
                            key.name,
                            self.usage_msg_from(&["use", &key.name])
                        );

                        key.name.clone()
                    }

                    Err(err) => {
                        eprintln!("Failed to add key: {err}");
                        return;
                    }
                };

                if *use_key {
                    store.use_key(&new_key_name).unwrap();
                    println!("Using key: {}", &new_key_name);
                }

                store.save().unwrap();
            }

            Command::Use { key_name } => match store.use_key(key_name) {
                Ok(Some(key)) => {
                    println!(
                        "Selected and now using key '{}', linked as SSH key.",
                        key.name
                    );

                    store.save().unwrap();
                }

                Ok(None) => {
                    eprintln!("{}", self.key_typo_msg_from(key_name));
                    return;
                }

                Err(err) => {
                    eprintln!("Failed to use key: {err}");
                    return;
                }
            },

            Command::Remove { key_name, force } => {
                let active_key = store.get_active_key();
                if let Some(key) = active_key {
                    let in_use = key.name == *key_name;

                    if in_use && !force {
                        eprintln!(
                            "The key '{key_name}' is currently in use, use --force to remove it"
                        );
                        return;
                    }
                }

                match store.remove_key(key_name) {
                    Some(key) => {
                        println!("Removed key: {}", key.name);
                        key
                    }

                    None => {
                        eprintln!("{}", self.key_typo_msg_from(key_name));
                        return;
                    }
                };

                store.save().unwrap();
            }

            Command::Rename { key_name, new_name } => match store.rename_key(key_name, new_name) {
                Some(key) => {
                    println!("Renamed key: {} -> {}", key_name, key.name);
                    store.save().unwrap();
                }

                None => {
                    eprintln!("{}", self.key_typo_msg_from(key_name));
                }
            },

            Command::Info { key_name } => {
                let key = key_name
                    .as_ref()
                    .and_then(|name| store.get_key(name))
                    .or_else(|| store.get_active_key());

                match (key_name, key) {
                    (_, Some(key)) => {
                        println!("Viewing Key '{}':", &key.name);

                        if let Some(ref private_key_path) = key.private_key_path {
                            println!("  Private Key: {}", private_key_path.to_string_lossy());
                        }

                        if let Some(ref public_key_path) = key.public_key_path {
                            println!("  Public Key: {}", public_key_path.to_string_lossy());
                        }
                    }

                    (Some(key_name), None) => {
                        eprintln!("{}", self.key_typo_msg_from(key_name));
                    }

                    (None, None) => {
                        command
                            .find_subcommand_mut("info")
                            .expect("failed to find `info` subcommand")
                            .print_help()
                            .expect("failed to print help");
                    }
                }
            }
        }
    }
}
