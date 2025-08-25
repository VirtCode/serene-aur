use std::{
    env,
    io::{stdin, stdout, Write},
    process::{exit, Command, Stdio},
};

use anyhow::{anyhow, Context, Result};
use chrono::Utc;
use colored::Colorize;

use crate::{action::pacman, config::Config, table::ago, web::requests};

/// prints the intro sequence which walks the user through adding the secret
pub fn intro() -> Result<()> {
    println!("Welcome to {}!", "Serene".bold());
    println!("This seems to be the first time you use the CLI on your host, let's get it set up!");

    // prompt the user for the server URL
    println!();
    println!("1. In order to use this CLI, you need to have a functioning instance of Serene.");
    println!("   If you need help to deploy the server, refer to the documentation below:");
    println!("   https://virtcode.github.io/serene-aur/deployment/readme");
    println!();
    println!("Please enter the http URL to your server:");

    let mut url = String::new();
    stdin().read_line(&mut url).context("couldn't read line from stdin")?;
    url = url.trim().to_owned();
    println!();

    // test connection
    let config = Config::empty(url.clone());
    let info = match requests::get_info(&config) {
        Ok(info) => info,
        Err(e) => {
            println!("Failed to reach your server at `{url}`!");
            println!("  ({})", e.msg());
            println!(
                "Make sure the URL is correct and that your server is online, then try again."
            );
            exit(1);
        }
    };

    println!("Successfully connected to your server!");
    println!(
        "It's running Serene {} and is up for {}.",
        info.version,
        ago::coarse(Utc::now() - info.started).trim()
    );

    // write config now
    let config = config.write().context("failed to create config")?;

    // check architecture compatibility
    let mut pacman = true;
    if env::consts::ARCH != info.architecture {
        println!(
            "However, the server builds packages for {}, but your host is {}.",
            env::consts::ARCH,
            info.architecture
        );
        println!("This means you won't be able to use these packages on this host.");

        pacman = false;
    }

    println!();
    let action =
        if info.readable { "add and maintain packages" } else { "see and change managed packages" };

    println!("2. To be able to {action} on this server, you need to be authenticated.");
    println!("   So add the following line to its {} file:", "authorized_secrets".italic());
    println!("   You can skip this step if you don't need to {action}.");
    println!();

    config.print_secret(true);
    println!();

    // set up pacman if supported
    'pacman: {
        if !pacman {
            break 'pacman;
        }

        println!("3. And finally, to use your server you need to configure pacman to use it.");
        println!("   This will allow you to install packages from it on this host.");
        println!("   The CLI can do the setup for you or you could do it manually instead:");
        println!("   https://virtcode.github.io/serene-aur/#_3-configuring-pacman");
        println!();

        // prompt the user for installation
        if !prompt("Do you want to configure it now?", true)? {
            break 'pacman;
        }
        println!();

        // check for config
        if !pacman::config().exists() {
            println!("Couldn't find pacman config, you'll have to configure it manually then.");
            break 'pacman;
        }

        // check if added
        if pacman::has_repo(&info.name) {
            println!("It looks like you have already added the repository on this host.");
            println!(
                "In this case you are already configured, but might have to fix things manually."
            );

            break 'pacman;
        }

        let mut signed = info.signed;

        if signed {
            println!("Your server supports signed packages, which pacman can verify.");
            signed &= prompt("Do you want to set that up too (recommended)?", true)?;
            println!();
        }

        if !signed && !url.starts_with("https") {
            println!(
                "You are trying to use your repository {} HTTPS nor package signatures!",
                "without".bold()
            );
            println!("This can leave you vulnerable to various attacks and is NOT recommended.");

            if !prompt("Are you sure you want to continue?", false)? {
                break 'pacman;
            }

            println!();
        }

        // write into pacman config
        println!("4. You are now going to modify your pacman configuration.");
        println!(
            "   This will prompt you for superuser privileges, and write to `/etc/pacman.conf`."
        );
        println!();

        let pacman_config = pacman::config_repo(&config, &info.name, signed);
        println!("{}", pacman_config.trim());
        if !prompt("Append this to `/etc/pacman.conf` with as root?", true)? {
            break 'pacman;
        }

        if let Err(e) = run_as_root_with_stdin(
            &config.elevator,
            &["tee", "-a", &pacman::config().to_string_lossy()],
            &pacman_config,
        ) {
            println!("Configuring failed, you'll have to do it manually.");
            println!("  ({e:#})");
            break 'pacman;
        }

        // configure signatures
        if signed {
            println!();
            println!("5. Now you have to add the server's key to your keyring.");
            println!(
                "   The CLI will only import it, you you'll have to sign it afterwards yourself."
            );
            println!("   It will now download the key and add it to your pacman keyring.");
            println!();

            if !import_pacman_key(&config, true)? {
                break 'pacman;
            }

            println!();
            println!("6. Almost there, you'll now have to sign the key so pacman will trust it.");
            println!("   First, run `pacman-key --list-keys` and identify the key of the server.");
            println!("   Then, trust the key with `pacman-key --lsign-key <server-key-id>`");
            println!("   After that, pacman should be ready to use the key for signatures.");
        }
    }

    println!();
    println!("You are now all set up!");
    println!("Why not run the following to see what packages your server currently is building:");
    println!();
    println!("serene list");

    Ok(())
}

pub fn import_pacman_key(config: &Config, intro: bool) -> Result<bool> {
    if !intro {
        println!("We'll now download and import the server's key into your pacman keyring.");
        println!("This will require root privileges.");
        println!();
    }

    let key = match requests::get_key(config) {
        Ok(key) => key,
        Err(e) => {
            println!("Failed to download key from server, you'll have fix that manually.");
            println!("  ({})", e.msg());
            return Ok(false);
        }
    };

    if !prompt("Do you want to import the key with `pacman-key --add` as root?", true)? {
        return Ok(false);
    }

    if let Err(e) = run_as_root_with_stdin(&config.elevator, &["pacman-key", "--add", "-"], &key) {
        println!("Import failed, you'll have to do it manually.");
        println!("  ({e:#})");
        return Ok(false);
    }

    if !intro {
        println!();
        println!("You will now have to sign the imported key yourself.");
        println!("To do that, run `pacman-key --list-keys` and identify the server's key.");
        println!("Then, run `pacman-key --lsign-key <found-key-id>` to trust your key locally.");
    }

    Ok(true)
}

fn prompt(prompt: &str, def: bool) -> Result<bool> {
    print!("{prompt} [{}] ", if def { "Y/n" } else { "y/N" });
    stdout().flush()?;

    let mut confirm = String::new();
    stdin().read_line(&mut confirm).context("couldn't read line from stdin")?;
    let confirm = confirm.trim().to_lowercase();

    Ok(if confirm.starts_with("y") {
        true
    } else if confirm.starts_with("n") {
        false
    } else {
        def
    })
}

fn run_as_root_with_stdin(elevator: &str, args: &[&str], input: &str) -> Result<()> {
    let readable = args.join(" ");

    let mut child = Command::new(elevator)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .spawn()
        .with_context(|| format!("failed to run `{readable}`"))?;

    let mut stdin = child.stdin.take().expect("stdin is piped");
    stdin.write_all(input.as_bytes()).context("failed to write to stdin")?;
    drop(stdin);

    let status =
        child.wait().with_context(|| format!("failed to wait for `{readable}` to exit"))?;
    if !status.success() {
        Err(anyhow!("failed to run `{readable}` successfully"))
    } else {
        Ok(())
    }
}
