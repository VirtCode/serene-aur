use std::{env, fs};
use std::io::Error;
use std::path::PathBuf;
use clap::{CommandFactory, ValueEnum};
use clap_complete::{Generator};

include!("src/command.rs");
include!("src/complete/script.rs");

fn main() -> Result<(), Error> {
    let Some(out) = env::var_os("COMPLETIONS_DIR").or_else(|| env::var_os("OUT_DIR")).map(PathBuf::from) else {
        return Ok(());
    };
    
    fs::create_dir_all(&out)?;

    for shell in Shell::value_variants() {
        let completions = generate_completions(*shell, "serene", &mut Args::command(), true);
        fs::write(out.join(shell.file_name("serene")), &completions)?;
    }

    Ok(())
}