use std::{collections::HashMap, error::Error, fs, io, path::PathBuf};

use serde::{Deserialize, Serialize};

use crate::platform;

// todo: make configurable
pub const DEFAULT_SSH_KEY_NAME: &str = "id_rsa";

pub const DEFAULT_JSON_FILE: &str = "keys.json";

pub fn get_folder() -> PathBuf {
    let mut home_folder = platform::get_home_folder();
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

        let store_path = get_keys_folder().join(&key_name).with_extension("");
        let key = Key {
            original_path: Some(path_to_key),
            private_key_path: Some(store_path),
            public_key_path: None,
            name: key_name.clone(),
        };

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
    /// Path to the original key file that was provided via CLI
    pub original_path: Option<PathBuf>,

    /// Path to the private key file in storage
    pub private_key_path: Option<PathBuf>,
    pub public_key_path: Option<PathBuf>,

    pub name: String,
}

impl Key {
    pub fn link(&self) -> Result<(), io::Error> {
        if self.private_key_path.as_ref().is_none() {
            return Ok(()); // nothing to link
        }

        let ssh_path = platform::get_ssh_path();
        let key_link_to = ssh_path.join(DEFAULT_SSH_KEY_NAME);

        platform::soft_link(self.private_key_path.as_ref().unwrap(), &key_link_to)?;

        Ok(())
    }

    pub fn delete(&self) -> Result<(), io::Error> {
        if let Some(path) = self.private_key_path.as_ref() {
            fs::remove_file(path)?;
        }

        if let Some(path) = self.public_key_path.as_ref() {
            fs::remove_file(path)?;
        }

        Ok(())
    }

    pub fn save(&self) -> Result<(), Box<dyn Error>> {
        // if the private key already exists, we don't need to save it again
        if self.private_key_path.as_ref().is_some_and(|p| p.exists()) {
            return Ok(());
        }

        // we can't copy to an empty path or if the original file doesn't exist
        if self.private_key_path.is_none()
            || self.original_path.as_ref().is_none_or(|p| !p.exists())
        {
            return Err("No private key path or the original file doesn't exist".into());
        }

        let original_path = self.original_path.as_ref().unwrap();
        let private_key_path = self.private_key_path.as_ref().unwrap();

        let save_to_folder = private_key_path.parent();
        if save_to_folder.as_ref().is_some_and(|p| !p.exists()) {
            fs::create_dir_all(save_to_folder.unwrap())?;
        }

        fs::copy(original_path, private_key_path)?;

        Ok(())
    }
}

// todo: impl Display for Key
