use std::{error::Error, path::PathBuf};

use clap::{Args, CommandFactory, Parser, Subcommand};

use crate::{error::CliError, store::SshKeyStorage};

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

#[derive(Debug, Clone, Args)]
pub struct AddArgs {
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
}

#[derive(Debug, Clone, Args)]
pub struct RenameArgs {
    key_name: String,
    new_name: String,
}

#[derive(Debug, Clone, Args)]
pub struct RemoveArgs {
    key_name: String,

    #[arg(
        short,
        long,
        help = "Force remove the key without confirmation when it is in use"
    )]
    force: bool,
}

#[derive(Debug, Clone, Subcommand)]
pub enum Command {
    #[command(
        name = "add",
        alias = "new",
        about = "Add a new SSH key with an existing private key",
        arg_required_else_help = true
    )]
    Add(AddArgs),

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
    Rename(RenameArgs),

    #[command(name = "remove", alias = "rm", about = "Remove a key by name")]
    Remove(RemoveArgs),

    #[command(name = "list", aliases = ["ls", "-l", "--list"], about = "List all keys")]
    List, // todo: options for output formatting
}

pub type SubcommandResult = Result<(), CliError>;

impl KeyManCli {
    pub fn usage_msg_from(&self, args: &[&str]) -> String {
        format!("{BIN_NAME} {}", args.join(" "))
    }

    pub fn handle_add(&self, args: &AddArgs, store: &mut SshKeyStorage) -> SubcommandResult {
        let key = store
            .add_key(args.private_key.clone(), args.name.as_deref())
            .map_err(|e| CliError::Message(e.to_string()))?;

        println!(
            "Added key '{}' to list of keys, use it with `{}`",
            key.name,
            self.usage_msg_from(&["use", &key.name])
        );

        // clone so the previous borrow 'ends' and we can use store.use_key
        let key_name = key.name.clone();

        if args.use_key {
            store
                .use_key(&key_name)
                .map_err(|e| CliError::Misc(e.into()))?;

            println!("Now using '{}'", &key_name);
        }

        store.save().map_err(CliError::SaveFailed)?;
        Ok(())
    }

    pub fn handle_rename(&self, args: &RenameArgs, store: &mut SshKeyStorage) -> SubcommandResult {
        let key = store
            .rename_key(&args.key_name, &args.new_name)
            .ok_or(CliError::KeyNotFound(args.key_name.clone()))?;

        println!("Renamed from '{}' -> '{}'", &args.key_name, key.name);
        store.save().map_err(CliError::SaveFailed)?;

        Ok(())
    }

    pub fn handle_list(&self, store: &SshKeyStorage) -> SubcommandResult {
        let current_key_name = store.get_active_key().map(|k| k.name.as_str());

        println!("Your SSH keys:");

        for &key in store.get_keys().iter() {
            let in_use = Some(key.name.as_str()) == current_key_name;

            println!("  - {}{}", key.name, if in_use { " (in use)" } else { "" });
        }

        Ok(())
    }

    pub fn handle_use(&self, key_name: &str, store: &mut SshKeyStorage) -> SubcommandResult {
        match store.use_key(key_name) {
            Ok(Some(key)) => {
                println!(
                    "Selected and now using key '{}', linked as SSH key",
                    key.name
                );

                store.save().map_err(CliError::SaveFailed)?;
                Ok(())
            }

            Ok(None) => Err(CliError::KeyNotFound(key_name.to_string())),
            Err(err) => Err(CliError::Misc(Box::new(err))),
        }
    }

    pub fn handle_info(&self, key_name: Option<&str>, store: &SshKeyStorage) -> SubcommandResult {
        let key = key_name
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

                Ok(())
            }

            (Some(key_name), None) => Err(CliError::KeyNotFound(key_name.to_string())),

            (None, None) => {
                Self::command()
                    .find_subcommand_mut("info")
                    .expect("failed to find `info` subcommand")
                    .print_help()
                    .expect("failed to print help");

                Ok(())
            }
        }
    }

    pub fn handle_remove(&self, args: &RemoveArgs, store: &mut SshKeyStorage) -> SubcommandResult {
        let active_key = store.get_active_key();

        if let Some(key) = active_key {
            let in_use = key.name == *args.key_name;

            if in_use && !args.force {
                return Err(CliError::Message(
                    "Use --force to remove a key that is currently in use".to_string(),
                ));
            }
        }

        match store.remove_key(&args.key_name) {
            Some(_) => {
                store.save().map_err(CliError::SaveFailed)?;

                println!("Successfully removed key '{}'", &args.key_name);

                Ok(())
            }

            None => Err(CliError::KeyNotFound(args.key_name.clone())),
        }
    }

    pub fn handle(&self) -> Result<(), Box<dyn Error>> {
        let mut command = Self::command();

        // parse the subcommand, otherwise print help
        // (I know clap provides a way to do this with macros, but
        // this gives more control in the future)
        let subcommand = match self.subcommand {
            Some(ref subcommand) => subcommand,
            None => {
                command.print_help().expect("failed to print help");
                return Ok(());
            }
        };

        // create the file or load the existing one
        let mut store = SshKeyStorage::from_default_file().unwrap_or_default();

        let result: SubcommandResult = match subcommand {
            Command::List => self.handle_list(&store),
            Command::Add(args) => self.handle_add(args, &mut store),
            Command::Use { key_name } => self.handle_use(key_name, &mut store),
            Command::Rename(args) => self.handle_rename(args, &mut store),
            Command::Remove(args) => self.handle_remove(args, &mut store),
            Command::Info { key_name } => self.handle_info(key_name.as_deref(), &store),
        };

        match result {
            Ok(_) => Ok(()),
            Err(err) => {
                eprintln!("{}", err.to_string());
                std::process::exit(1);
            }
        }
    }
}
