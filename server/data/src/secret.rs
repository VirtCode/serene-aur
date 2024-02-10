use base64::Engine;
use base64::prelude::BASE64_STANDARD;
use sha2::{Digest, Sha256};

pub fn hash(secret: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(secret);

    BASE64_STANDARD.encode(hasher.finalize())
}