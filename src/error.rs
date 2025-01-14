use std::error::Error;

#[derive(Debug)]
pub enum CliError {
    /// When the key is not found by the name provided, first positional is the key name
    KeyNotFound(String),

    /// When the store fails to save the key, first positional is the error
    SaveFailed(Box<dyn Error>),

    /// A custom message to display to the user under error conditions
    Message(String),

    /// Any error that will be conveyed to the user via its ToString implementation
    Misc(Box<dyn Error>),
}

impl From<&str> for CliError {
    fn from(msg: &str) -> Self {
        CliError::Message(msg.to_string())
    }
}

impl From<String> for CliError {
    fn from(msg: String) -> Self {
        CliError::Message(msg)
    }
}

impl ToString for CliError {
    fn to_string(&self) -> String {
        match *self {
            CliError::KeyNotFound(ref key_name) => {
                format!("No key named '{}' was found, maybe a typo?", key_name)
            }

            CliError::SaveFailed(ref err) => format!("Failed to save changes: {err:#}"),
            CliError::Message(ref msg) => msg.clone(),
            CliError::Misc(ref err) => format!("An unknown error has occurred: {err:#}"),
        }
    }
}
