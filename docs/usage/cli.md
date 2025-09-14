# Command Line Interface
The CLI is the main way to interact with Serene. It uses the API of the server to add and manipulate the packages that are managed by the server. Note that the CLI **has nothing to do with installing the packages**, this will be taken care of by normal `pacman` as serene offers an ordinary package repository.

## Overview
This section serves as a small overview to get an idea of what the CLI is capable of. To get exhaustive options and information about a (sub-) command, you should still refer to the `--help` page of that item, as only some options are shown here.

**Listing all packages:** To list all packages built by the server, their version and build status, use:
```shell
# List all managed packages:
serene list
```

**Adding packages:** Serene currently supports three types of packages, those from the AUR, custom git repositories, and custom PKGBUILDs. The custom git repositories must be formulated the same way as AUR repositories are. An added package is built immediately. Supplying `--replace` replaces the source of a package if it is already added, which is often used when updating custom pkgbuilds. Adding them is straight forward:
```shell
# Adding an AUR package called `my-package`.
serene add my-package

# Adding a custom repository from github. The optional devel flag specifies that it is a development package (e.g. it works like -git).
serene add --custom --devel https://github.com/my-user/my-package

# Adding a custom pkgbuild for a git package from the filesystem, replacing the previous version. We load the pkgbuild from the filesystem.
serene add --pkgbuild --devel --replace --file ./PKGBUILD

# Adding and automatically installing `my-package` from the AUR without showing build logs.
serene add --install --quiet my-pacakge
```

**Removing packages:** To remove a package, just call the remove subcommand with the package base:
```shell
# Removes `my-package`.
serene remove my-package
```

**Build package now:** To build a package now, run this subcommand with the package base:
```shell
# Builds `my-package` and `my-other-package` simultaneously now.
serene build my-package my-other-package

# Builds `my-package` in a clean container and install it now.
serene build --clean --install my-package

# Build all added packages now, if not up-to-date.
serene build --all
```

**See package information:** To see all information for a package, you can use the info command and its various subcommands:
```shell
# Get an overview about the package `my-package`. Add `--all` to see all, and not just the latest eight builds.
serene info my-package

# Print the PKGBUILD used for the package currently in the repository to stdout.
serene info my-package pkgbuild

# See more information about the latest build. Supply an id for a specific one.
serene info my-package build

# See the logs of the latest build. Supply an id for a specific one. Add `--subscribe` to get live logs until next build is finished and `--linger` to indefinitely attach to live logs.
serene info my-package logs
```

**Changing package properties:** To change properties of a package and how it is built, you can use the set subcommand of info:
```shell
# Enable clean build, meaning the container is removed after each build and recreated. Any boolean value can be supplied.
serene info my-package set clean true

# Disable automatic building of the package. Any boolean value can be supplied.
serene info my-package set enable false

# Set a custom schedule for this package only. Expects a valid cron string.
serene info my-package set schedule "0 * * * *"

# Set commands to run before the package is built. This is mainly used to add e.g. required keys, or change something else about the container. It will be executed with bash.
serene info my-package set prepare "echo 'i am run before the makepkg'"

# Set additional flags which are passed to makepkg when building. See `makepkg --help` for more information. Note that only some options are supported.
serene info my-package set flags "nocheck" "holdver"

# Change whether the package is purely added as a dependency
serene info my-package set dependency false

# Mark the packages set prepare commands as private
serene info my-package set private true
```

**Manage the server**: To manage some server properties, you can use the server subcommand:

```shell
# See general server information
serene server info

# Request and print the webhook secret for the package `my-package`
serene server webhook my-package

# Get the public key of the server easily, in a machine-readable way
serene server key --machine
```

**Configure your host:** To make things easier on your host, you can use the host subcommand:
```shell
# Prints the secret to add to the `authorized_secrets` file.
serene secret

# Walks you thorugh setting up package signature verification on your host.
serene host signatures
```

## Configuration
The CLI does not offer much local configuration. It does set up everything needed automatically on the first startup, like prompting the user for the location of the server, generating a secret, etc. This makes it very easy to set up.

This configuration is stored at `~/.config/serene.yml` (or wherever your `XDG_CONFIG_HOME` is) as YAML. It contains the following attributes and defaults:
```yaml
# url of the server that is used
url: <my-server-url>

# root elevator binary that is used
elevator: sudo
```

The secret is stored seperately at `~/.local/share/serene/secret.txt` as plain text. This is done such that you can easier version-control your Serene config as part of some dotfiles. If you have started to use Serene on an older version, your secret might be still stored in the config with the `secret:` attribute. Secrets stored there will override the one in the secret file, but using that is discouraged.

## Shell Completions
As seen in the [Building Manually](#building-manually) section, the cli includes shell completions. These will be installed automatically for most shells (unless you're [building directly](#building-directly)). Make sure to enable them in your preferred shell.

Some shells (currently only Bash anz Zsh) also feature dynamic completions. This means you can complete package names for commands like `serene info` or `serene build`. The names that can be completed are sourced from a cache file, which is updated every time `serene list` is invoked. This avoids unintended network usage when just completing your command, but might not be totally accurate.

So if your package completions are not up-to-date, make sure your shell supports them and run `serene list` to update the completions in the background. Contributions for support for other shells are welcomed.
