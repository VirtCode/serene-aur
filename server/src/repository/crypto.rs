use crate::config::CONFIG;
use crate::repository::{GPG_AGENT_SOCKET, KEY_FILE};
use anyhow::Context;
use log::warn;
use sequoia_gpg_agent::keyinfo::KeyProtection;
use sequoia_gpg_agent::sequoia_ipc::Keygrip;
use sequoia_gpg_agent::{self as gpg_agent, Agent, PinentryMode};
use sequoia_openpgp::crypto::{self, Password, Signer};
use sequoia_openpgp::parse::Parse;
use sequoia_openpgp::policy::StandardPolicy;
use sequoia_openpgp::serialize::stream::{self, Message};
use sequoia_openpgp::serialize::Serialize;
use sequoia_openpgp::Cert;
use std::path::Path;
use std::{fs, io};

pub fn should_sign_packages() -> bool {
    Path::new(KEY_FILE).exists()
}

fn get_local_keypair() -> anyhow::Result<crypto::KeyPair> {
    let cert = Cert::from_file(KEY_FILE).context("failed to read private key file")?;
    let policy = StandardPolicy::new();

    let key = cert
        .keys()
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
        let password = Password::from(
            CONFIG
                .sign_key_password
                .clone()
                .context("private key is encrypted but no password was provided")?,
        );
        let algo = key.pk_algo();
        key.secret_mut()
            .decrypt_in_place(algo, &password)
            .context("failed to unlock private key")?;
    };

    key.into_keypair().context("failed to create keypair")
}

struct CHVStatus {
    chv1_cached: bool,
    chvmaxlen: [u32; 3],
    chvretry: [u32; 3],
}

impl CHVStatus {
    fn from_string(s: &str) -> anyhow::Result<Self> {
        fn unescape<S: AsRef<str>>(s: S) -> anyhow::Result<String> {
            let mut r = String::with_capacity(s.as_ref().len());
            let mut chars = s.as_ref().chars();
            while let Some(c) = chars.next() {
                if c == '%' {
                    let n = chars.next().context("unexpected end of string")?;
                    let m = chars.next().context("unexpected end of string")?;
                    let n = u8::from_str_radix(&format!("{}{}", n, m), 16)
                        .context("failed to parse hex value")?;
                    r.push(n as char);
                } else if c == '+' {
                    r.push(' ');
                } else {
                    r.push(c);
                }
            }
            Ok(r)
        }

        // split and parse as integers
        let parts = unescape(s)
            .context("failed to unescape CHVStatus")?
            .split_ascii_whitespace()
            .map(|part| u32::from_str_radix(part, 10))
            .collect::<Result<Vec<_>, _>>()
            .context("failed to parse CHVStatus")?;

        if parts.len() < 7 {
            return Err(anyhow::anyhow!("CHVStatus has too few parts"));
        }

        Ok(Self {
            chv1_cached: parts[0] == 1,
            chvmaxlen: [parts[1], parts[2], parts[3]],
            chvretry: [parts[4], parts[5], parts[6]],
        })
    }

    fn from_card_info(card_info: &gpg_agent::cardinfo::CardInfo) -> anyhow::Result<Option<Self>> {
        card_info
            .raw()
            .find_map(|(keyword, value)| match keyword.as_str() {
                "CHV-STATUS" => Some(Self::from_string(&value)),
                _ => None,
            })
            .transpose()
    }
}

async fn get_agent_keypair() -> anyhow::Result<gpg_agent::KeyPair> {
    let cert = Cert::from_file(KEY_FILE).context("Failed to read public key file")?;
    let policy = StandardPolicy::new();

    let mut agent = Agent::connect_to_agent(GPG_AGENT_SOCKET)
        .await
        .context("Failed to connect to gpg-agent")?
        .suppress_pinentry();

    let card_info = match agent.card_info().await {
        Ok(info) => Some(info),
        Err(e) => {
            warn!("Failed to get card info: {e}");
            None
        }
    };

    let keys = agent.list_keys().await.context("Failed to list keys")?;

    let (key, agent_key) = cert
        .keys()
        .with_policy(&policy, None)
        .supported()
        .alive()
        .revoked(false)
        .for_signing()
        .map(|k| k.key())
        .find_map(|k| {
            if let Ok(keygrip) = Keygrip::of(k.mpis()) {
                return keys
                    .iter()
                    // we can't use keys that require user confirmation
                    .filter(|key| !key.confirmation_required() && !key.key_disabled())
                    .find(|key| key.keygrip() == &keygrip)
                    .map(|key| (k, key));
            }
            None
        })
        .context("No suitable key found in gpg-agent")?;

    if let Some(ci) = card_info {
        // check if the key we are using is on the card
        if ci.keys_keygrips().any(|kg| &kg == agent_key.keygrip()) {
            let chv_status = CHVStatus::from_card_info(&ci).context("Failed to parse CHVStatus")?;

            // check the remaining retries for the user PIN
            match chv_status {
                Some(chv_status) if chv_status.chvretry[0] > 1 => Ok(()),
                Some(chv_status) if chv_status.chvretry[0] == 1 => Err(anyhow::anyhow!(
                    "user PIN has one retry left, refusing to use it to avoid lockout"
                )),
                Some(chv_status) => Err(anyhow::anyhow!("card is locked")),
                None => {
                    warn!("Failed to get CHVStatus from card info");
                    Ok(())
                }
            }?;
        }
    }

    let keypair = agent.keypair(&key).context("Failed to create keypair")?;

    match &CONFIG.sign_key_password {
        Some(password) => {
            let password = Password::from(password.clone());
            let keypair = keypair.set_pinentry_mode(PinentryMode::Loopback).with_password(password);

            Ok(keypair)
        }
        None => match agent_key.protection() {
            KeyProtection::NotProtected => Ok(keypair),
            KeyProtection::Protected => {
                Err(anyhow::anyhow!("private key is protected but no password was provided"))
            }
            KeyProtection::UnknownProtection => Ok(keypair), // will likely fail but try anyway
            _ => Err(anyhow::anyhow!("unsupported key protection")),
        },
    }
}

async fn get_keypair() -> anyhow::Result<Box<dyn Signer + Send + Sync>> {
    if Path::new(GPG_AGENT_SOCKET).exists() {
        Ok(Box::new(get_agent_keypair().await?))
    } else {
        Ok(Box::new(get_local_keypair()?))
    }
}

pub async fn sign(output: &Path, file: &Path) -> anyhow::Result<()> {
    let keypair = get_keypair().await.context("failed to get keypair")?;
    let mut sink = fs::File::create(output).context("failed to create signature sink")?;
    let message = Message::new(&mut sink);

    let mut message = stream::Signer::new(message, keypair)
        .detached()
        .build()
        .context("failed to create signer")?;
    let mut source = fs::File::open(file).context("failed to open source file")?;
    io::copy(&mut source, &mut message).context("failed to sign file")?;

    message.finalize().context("failed to write signature file")?;
    Ok(())
}

pub fn get_public_key_bytes<W: io::Write + Send + Sync>(output: &mut W) -> anyhow::Result<()> {
    let cert = Cert::from_file(KEY_FILE).context("failed to read private key file")?;
    // this behavior is not very well documented from the sequoia side. The code to
    // serialize public keys can be found in the code for the `sq` cli here: https://gitlab.com/sequoia-pgp/sequoia-sq/-/blame/main/src/commands/key/export.rs?ref_type=heads#L103
    let mut writer =
        sequoia_openpgp::armor::Writer::new(output, sequoia_openpgp::armor::Kind::PublicKey)
            .context("failed to build public key armorer")?;

    cert.serialize(&mut writer).context("failed to export public key")?;
    writer.finalize()?;
    Ok(())
}
