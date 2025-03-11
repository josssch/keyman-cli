use ssh_key::{PrivateKey as OpenSshPrivateKey, PublicKey};

use crate::error::CliError;

pub const RSA_HEADER: &str = "-----BEGIN RSA PRIVATE KEY-----";
pub const OPENSSH_HEADER: &str = "-----BEGIN OPENSSH PRIVATE KEY-----";

pub enum PrivateKey {
    Open(OpenSshPrivateKey),
}

impl PrivateKey {
    pub fn public_key(&self) -> &PublicKey {
        match *self {
            PrivateKey::Open(ref key) => key.public_key(),
        }
    }
}

pub fn decode_from_str(key: &str) -> Result<PrivateKey, CliError> {
    let mut lines = key.trim().lines();

    // todo: support more key formats
    match lines.next() {
        Some(OPENSSH_HEADER) => {
            let key =
                OpenSshPrivateKey::from_openssh(key).map_err(|_| CliError::UnsupportedKeyFormat)?;

            Ok(PrivateKey::Open(key))
        }

        _ => Err(CliError::UnsupportedKeyFormat),
    }
}
