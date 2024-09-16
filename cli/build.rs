use clap::{CommandFactory, ValueEnum};
use clap_complete::Generator;
use std::io::Error;
use std::path::PathBuf;
use std::{env, fs, process};

include!("src/command.rs");
include!("src/complete/script.rs");

fn main() -> Result<(), Error> {
    shell_completions()?;
    git_version()?;

    Ok(())
}

fn git_version() -> Result<(), Error> {
    let git = process::Command::new("git").arg("describe").arg("--tags").arg("--abbrev=0").output();

    let tag = match git {
        Ok(o) => String::from_utf8_lossy(&o.stdout).to_string(),
        Err(e) => {
            println!("cargo::warning=Failed to find tag for version: {e:#}");
            "untagged".to_string()
        }
    };

    println!("cargo::rustc-env=TAG={tag}");

    Ok(())
}

/// generate shell version
fn shell_completions() -> Result<(), Error> {
    let Some(out) =
        env::var_os("COMPLETIONS_DIR").or_else(|| env::var_os("OUT_DIR")).map(PathBuf::from)
    else {
        return Ok(());
    };

    fs::create_dir_all(&out)?;

    for shell in Shell::value_variants() {
        let completions = generate_completions(*shell, "serene", &mut Args::command(), true);
        fs::write(out.join(shell.file_name("serene")), &completions)?;
    }

    Ok(())
}
