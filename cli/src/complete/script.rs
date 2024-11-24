use clap::Command;
use clap_complete::Shell;
use std::io::BufWriter;

#[rustfmt::skip]
const PACKAGE_COMPLETION_COMMANDS: [(&str, bool); 4] = [
    ("info",            false), 
    ("build",           true), 
    ("remove",          false), 
    ("manage webhook",  false),
];

pub fn generate_completions(
    shell: Shell,
    binary: &str,
    command: &mut Command,
    warnings: bool,
) -> String {
    let mut buffer = BufWriter::new(Vec::new());
    clap_complete::generate(shell, command, binary, &mut buffer);

    let mut output =
        String::from_utf8_lossy(&buffer.into_inner().unwrap_or(Vec::new())).to_string();

    // yes this currently only supports bash, feel free to contribute other shells!
    if matches!(shell, Shell::Bash) {
        for (tokens, multiple) in PACKAGE_COMPLETION_COMMANDS {
            let search = format!("{binary}__{})", tokens.replace(" ", "__")); // case in big switch

            if let Some(pos) = output
                .find(&search)
                .and_then(|pos| output[pos..].find("opts=").map(|n| pos + n))
                .and_then(|pos| output[pos..].find("\n").map(|n| pos + n + 1))
            {
                output.insert_str(
                    pos,
                    &package_completion_bash(tokens.split(" ").count() as u32 + 1, multiple),
                );
            } else if warnings {
                println!("cargo:error=did not find '{search}' in completions, did the completions change?");
            }
        }
    }

    output
}

pub fn package_completion_bash(level: u32, multiple: bool) -> String {
    let base = r#"
        if [[ ${cur} != -* %%CONDITIONS%% ]] ; then
            local packages path

            path="$XDG_CACHE_HOME"
            [[ -n "$path" ]] || path="$HOME/.cache"
            path="$path/serene-package-completions.txt"

            packages="<NAME>"
            [[ -e "$path" ]] && packages="$(cat $path)"

            COMPREPLY=( $(compgen -W "$packages %%OPTIONS%%" -- "${cur}") )
            return 0
        fi
    "#;

    if multiple {
        base.replace("%%CONDITIONS%%", "").replace("%%OPTIONS%%", "$opts")
    } else {
        base.replace("%%CONDITIONS%%", &format!("&& ${{COMP_CWORD}} -eq {level}"))
            .replace("%%OPTIONS%%", "")
    }
}
