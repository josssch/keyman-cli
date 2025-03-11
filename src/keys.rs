use ssh_key::{Fingerprint, HashAlg, PrivateKey as OpenSshPrivateKey, PublicKey};

use crate::error::CliError;

pub const OPENSSH_HEADER: &str = "-----BEGIN OPENSSH PRIVATE KEY-----";

pub enum PrivateKey {
    OpenSsh(OpenSshPrivateKey),
}

impl PrivateKey {
    pub fn fingerprint(&self) -> Fingerprint {
        match *self {
            PrivateKey::OpenSsh(ref key) => key.fingerprint(HashAlg::Sha256),
        }
    }

    pub fn public_key(&self) -> &PublicKey {
        match *self {
            PrivateKey::OpenSsh(ref key) => key.public_key(),
        }
    }
}

impl TryFrom<&str> for PrivateKey {
    type Error = CliError;

    fn try_from(key: &str) -> Result<Self, Self::Error> {
        let mut lines = key.trim().lines();

        // todo: support more key formats
        match lines.next() {
            Some(OPENSSH_HEADER) => {
                let key = OpenSshPrivateKey::from_openssh(key)
                    .map_err(|_| CliError::UnsupportedKeyFormat)?;

                Ok(PrivateKey::OpenSsh(key))
            }

            _ => Err(CliError::UnsupportedKeyFormat),
        }
    }
}
