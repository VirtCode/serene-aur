use std::{env, fs};
use std::path::PathBuf;
use clap::CommandFactory;
use clap_complete::Shell;
use serene_data::package::PackagePeek;
use crate::command::Args;
use crate::log::Log;

mod script;

pub fn save_completions(packages: &[PackagePeek]) {
    let Ok(path) = env::var("XDG_CACHE_HOME").map(PathBuf::from)
        .or_else(|_| env::var("HOME").map(|h| PathBuf::from(h).join(".cache"))) else {

        Log::warning("could not find cache dir for package completions");
        return;
    };

    if let Err(e) = fs::write(path.join("serene-package-completions.txt"),
                              packages.iter().map(|p| p.base.clone()).intersperse(" ".to_string()).collect::<String>()
    ) {
        Log::warning(&format!("could not store package completions: {e:#}"));
    }
}

pub fn generate_completions(shell: Shell) -> String {
    script::generate_completions(shell, "serene", &mut Args::command(), false)
}
