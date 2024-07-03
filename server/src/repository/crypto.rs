use std::{fs, io};
use std::path::Path;
use anyhow::{Context};
use sequoia_openpgp::Cert;
use sequoia_openpgp::crypto::{KeyPair, Password};
use sequoia_openpgp::parse::Parse;
use sequoia_openpgp::policy::StandardPolicy;
use sequoia_openpgp::serialize::{Serialize};
use sequoia_openpgp::serialize::stream::{Armorer, Message, Signer};
use crate::config::CONFIG;
use crate::repository::PRIV_KEY_FILE;

pub fn should_sign_packages() -> bool {
    Path::new(PRIV_KEY_FILE).exists()
}

fn get_keypair() -> anyhow::Result<KeyPair> {
    let cert = Cert::from_file(PRIV_KEY_FILE).context("failed to read private key file")?;
    let policy = StandardPolicy::new();

    let key = cert.keys()
        .secret()
        .with_policy(&policy, None)
        .supported()
        .alive()
        .revoked(false)
        .for_signing()
        .next()
        .context("certificate should contain at least one key")?;

    let mut key = key.key().clone();
    if key.secret().is_encrypted() {
        let password = Password::from(CONFIG.sign_key_password.clone().context("private key is encrypted but no password was provided")?);
        let algo = key.pk_algo();
        key.secret_mut()
            .decrypt_in_place(algo, &password)
            .context("failed to unlock private key")?;
    };

    key.into_keypair().context("failed to create keypair")
}

pub fn sign(output: &Path, file: &Path) -> anyhow::Result<()> {
    let keypair = get_keypair()?;
    let mut sink = fs::File::create(output).context("failed to create signature sink")?;
    let message = Message::new(&mut sink);
    let message = Armorer::new(message)
        .kind(sequoia_openpgp::armor::Kind::Signature)
        .build()
        .context("failed to build signature armorer")?;

    let mut message = Signer::new(message, keypair).detached().build().context("failed to create signer")?;
    let mut source = fs::File::open(file).context("failed to open source file")?;
    io::copy(&mut source, &mut message).context("failed to sign file")?;

    message.finalize().context("failed to write signature file")?;
    Ok(())
}

pub fn get_public_key_bytes<W: io::Write + Send + Sync>(output: &mut W) -> anyhow::Result<()> {
    let cert = Cert::from_file(PRIV_KEY_FILE).context("failed to read private key file")?;
    // this behaviour is not very well documented from the sequoia side. The code to serialize public keys can be found in the code for the `sq` cli here:
    // https://gitlab.com/sequoia-pgp/sequoia-sq/-/blame/main/src/commands/key/export.rs?ref_type=heads#L103
    let mut writer = sequoia_openpgp::armor::Writer::new(output, sequoia_openpgp::armor::Kind::PublicKey)
        .context("failed to build public key armorer")?;

    cert.as_tsk().serialize(&mut writer).context("failed to export public key")?;
    writer.finalize()?;
    Ok(())
}