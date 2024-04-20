use std::io::BufWriter;
use clap::Command;
use clap_complete::Shell;

const PACKAGE_COMPLETION_BASH: &str = r#"
if [[ ${cur} != -* && ${COMP_CWORD} -eq 2 ]] ; then
    local packages path

    path="$XDG_CACHE_HOME"
    [[ -n "$path" ]] || path="$HOME/.cache"
    path="$path/serene-package-completions.txt"

    packages="<NAME>"
    [[ -e "$path" ]] && packages="$(cat $path)"

    COMPREPLY=( $(compgen -W "$packages" -- "${cur}") )
    return 0
fi
"#;

const PACKAGE_COMPLETION_COMMANDS: [&str; 3] = [
    "info",
    "build",
    "remove"
];


pub fn generate_completions(shell: Shell, binary: &str, command: &mut Command, warnings: bool) -> String {
    let mut buffer = BufWriter::new(Vec::new());
    clap_complete::generate(shell, command, binary, &mut buffer);

    let mut output = String::from_utf8_lossy(&*buffer.into_inner().unwrap_or(Vec::new())).to_string();

    // yes this currently only supports bash, feel free to contribute other shells!
    if matches!(shell, Shell::Bash) {
        for command in PACKAGE_COMPLETION_COMMANDS {
            let search = format!("{binary}__{command})"); // case in big switch

            if let Some(mut pos) = output.find(&search) {
                pos += search.len();

                output.insert_str(pos, PACKAGE_COMPLETION_BASH);
            } else if warnings {
                println!("cargo:error=did not find '{search}' in completions, did the completions change?");
            }
        }
    }

    output
}