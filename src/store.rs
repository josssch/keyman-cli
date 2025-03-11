use std::{cell::Cell, collections::HashMap, error::Error, fmt::Display, fs, io, path::PathBuf};

use serde::{Deserialize, Serialize};

use crate::{error::CliError, keys::PrivateKey, platform};

// todo: make configurable
pub const DEFAULT_SSH_KEY_NAME: &str = "id_rsa";

pub const DEFAULT_JSON_FILE: &str = "keys.json";

pub fn get_folder() -> PathBuf {
    // use the current working directory if not in debug mode
    let mut home_folder = if cfg!(not(debug_assertions)) {
        platform::get_home_folder()
    } else {
        PathBuf::new()
    };

    home_folder.push(format!(".{}", env!("CARGO_PKG_NAME")));

    home_folder
}

pub fn get_keys_folder() -> PathBuf {
    get_folder().join("keys")
}

pub fn create_folders() -> Result<(), io::Error> {
    let folder = get_folder();
    if !folder.exists() {
        fs::create_dir_all(&folder)?;
    }

    let keys_folder = get_keys_folder();
    if !keys_folder.exists() {
        fs::create_dir_all(&keys_folder)?;
    }

    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SshKeyStorage {
    #[serde(default = "serde_default_file_name", skip)]
    pub file_name: String,

    #[serde(default, skip)]
    marked_for_deletion: Vec<Key>,

    active_key_name: Option<String>,
    keys_by_name: HashMap<String, Key>,
}

fn serde_default_file_name() -> String {
    DEFAULT_JSON_FILE.to_string()
}

impl Default for SshKeyStorage {
    fn default() -> Self {
        Self {
            active_key_name: None,
            file_name: DEFAULT_JSON_FILE.to_string(),
            keys_by_name: HashMap::new(),
            marked_for_deletion: Vec::new(),
        }
    }
}

impl SshKeyStorage {
    pub fn from_default_file() -> Option<Self> {
        let file_path = get_folder().join(DEFAULT_JSON_FILE);
        if !file_path.exists() {
            return None;
        }

        let file = fs::File::open(&file_path).ok()?;
        serde_json::from_reader(file).ok()
    }

    #[allow(unused)]
    pub fn new() -> Self {
        Default::default()
    }

    pub fn default_next_name(&self) -> String {
        loop {
            let name = format!("key{}", self.keys_by_name.len());

            if !self.keys_by_name.contains_key(&name) {
                return name;
            }
        }
    }

    pub fn get_active_key(&self) -> Option<&Key> {
        self.active_key_name
            .as_ref()
            .and_then(|name| self.keys_by_name.get(name))
    }

    pub fn get_keys(&self) -> Vec<&Key> {
        self.keys_by_name.values().collect()
    }

    pub fn get_key(&self, name: &str) -> Option<&Key> {
        self.keys_by_name.get(name)
    }

    pub fn use_key(&mut self, name: &str) -> Result<Option<&Key>, io::Error> {
        if !self.keys_by_name.contains_key(name) {
            return Ok(None);
        }

        self.active_key_name = Some(name.to_string());

        let active_key = self
            .get_active_key()
            .expect("active key was just set, should not be None");

        active_key.link()?;

        Ok(Some(active_key))
    }

    pub fn add_key(
        &mut self,
        path_to_key: PathBuf,
        name: Option<&str>,
    ) -> Result<&Key, Box<dyn Error>> {
        if !path_to_key.is_file() {
            return Err("invalid path to private key".into());
        }

        let key_name = name.map_or(
            path_to_key
                .file_stem()
                .map_or(self.default_next_name(), |s| {
                    s.to_string_lossy().to_string()
                }),
            ToString::to_string,
        );

        if self.keys_by_name.contains_key(&key_name) {
            return Err("key with that name already exists".into());
        }

        let key = Key::new(&key_name, path_to_key);
        self.keys_by_name.insert(key_name.clone(), key);

        Ok(self
            .keys_by_name
            .get(&key_name)
            .expect("key was just added"))
    }

    pub fn remove_key(&mut self, name: &str) -> Option<&Key> {
        let key = match self.keys_by_name.remove(name) {
            Some(key) => key,
            None => return None,
        };

        if self
            .active_key_name
            .as_ref()
            .is_some_and(|name| name == &key.name)
        {
            self.active_key_name = None;
        }

        self.marked_for_deletion.push(key);
        self.marked_for_deletion.last()
    }

    pub fn rename_key(&mut self, name: &str, new_name: &str) -> Option<&Key> {
        if !self.keys_by_name.contains_key(name) {
            return None;
        }

        let mut key = self.keys_by_name.remove(name).unwrap();

        let new_name = new_name.to_string();
        key.name = new_name.clone();

        if self.active_key_name.as_ref().is_some_and(|n| n == name) {
            self.active_key_name = Some(new_name.clone());
        }

        self.keys_by_name.insert(new_name.clone(), key);
        self.keys_by_name.get(&new_name)
    }

    pub fn save(&mut self) -> Result<PathBuf, Box<dyn Error>> {
        create_folders()?;

        let folder = get_folder();

        let output_path = folder.join(&self.file_name);
        let file = fs::File::create(&output_path)?;
        serde_json::to_writer_pretty(file, &self)?;

        // save all of the keys
        for key in self.keys_by_name.values() {
            key.save()?;
        }

        for key in &self.marked_for_deletion {
            key.delete()?;
        }

        self.marked_for_deletion.clear();

        Ok(output_path)
    }
}

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Key {
    /// The name and identifier of this key
    pub name: String,

    /// Path to the original key file that was provided via CLI
    pub original_private_key_path: PathBuf,

    /// Path to the private key file in storage
    pub private_key_path: PathBuf,

    /// Path to the public key file in storage
    pub public_key_path: PathBuf,

    /// Whether the keys are saved on disk or not
    #[serde(skip)]
    pub is_saved: Cell<bool>,
}

impl Key {
    pub fn new(name: &str, private_key: PathBuf) -> Self {
        let private_key_path = get_keys_folder().join(name).with_extension("");

        Self {
            name: name.to_string(),
            original_private_key_path: private_key,
            public_key_path: private_key_path.with_extension("pub"),
            private_key_path,
            is_saved: Cell::new(false),
        }
    }

    pub fn private_key(&self) -> Result<PrivateKey, CliError> {
        if !self.private_key_path.is_file() {
            return Err("private key must be saved first".into());
        }

        let private_key_contents =
            fs::read_to_string(&self.private_key_path).map_err(|e| CliError::Misc(Box::new(e)))?;

        PrivateKey::try_from(private_key_contents.as_str())
    }

    pub fn fingerprint(&self) -> Option<String> {
        Some(self.private_key().ok()?.fingerprint().to_string())
    }

    pub fn link(&self) -> Result<(), io::Error> {
        let ssh_folder = platform::get_ssh_path();

        let ssh_private_key = ssh_folder.join(DEFAULT_SSH_KEY_NAME);
        let ssh_public_key = ssh_private_key.with_extension("pub");

        platform::soft_link(&self.private_key_path, &ssh_private_key)?;
        platform::soft_link(&self.public_key_path, &ssh_public_key)?;

        Ok(())
    }

    pub fn delete(&self) -> Result<(), io::Error> {
        fs::remove_file(&self.private_key_path)?;
        fs::remove_file(&self.public_key_path)?;
        Ok(())
    }

    pub fn save(&self) -> Result<(), Box<dyn Error>> {
        if !self.original_private_key_path.is_file() {
            return Err("original private key path is not a file".into());
        }

        let parent_folder = self.private_key_path.parent();
        if let Some(parent_folder) = parent_folder {
            if !parent_folder.exists() {
                fs::create_dir_all(parent_folder)?;
            }
        }

        if !self.is_saved.get() {
            // must copy the original private key to the storage location
            fs::copy(&self.original_private_key_path, &self.private_key_path)?;

            let private_key = self
                .private_key()
                .map_err(|_| "failed to load private key")?;

            let public_key = private_key.public_key();
            public_key.write_openssh_file(&self.public_key_path)?;

            self.is_saved.set(true);
        }

        Ok(())
    }
}

impl Display for Key {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}{}",
            self.name,
            self.fingerprint()
                // add a prefixing space if it's not empty
                .map(|f| format!(" {f}"))
                .unwrap_or_default()
        )
    }
}
