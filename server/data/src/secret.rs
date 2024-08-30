use base64::Engine;
use base64::prelude::{BASE64_STANDARD, BASE64_URL_SAFE};
use sha2::{Digest, Sha256};

/// hashes a secret and converts it to string, the way it is in authorized_secrets
pub fn hash(secret: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(secret);

    BASE64_STANDARD.encode(hasher.finalize())
}

/// hash a secret and convert it to a string which is url safe <br />
/// this is especially needed for webhook secrets which are delivered in a query parameter
pub fn hash_url_safe(secret: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(secret);
    
    BASE64_URL_SAFE.encode(hasher.finalize())
}