use std::process::Command;
use std::io::Error;

fn main() -> Result<(), Error> {
    git_version()?;

    Ok(())
}

fn git_version() -> Result<(), Error> {
    let git = Command::new("git")
            .arg("describe")
            .arg("--tags")
            .arg("--abbrev=0")
            .output();

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
