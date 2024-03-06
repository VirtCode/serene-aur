# Command Line Interface
The CLI is the main way to interact with a serene server. It uses the [API](../server/README.md#api) of the server to add and manipulate the packages that are managed by the server. It is not required to [just use the built packages](../README.md#installing-only-the-repository) from the server though, as this is purely done through pacman.

## Usage
This section will cover all the ways one can use the cli to interact with the server. Note that a help page is available for every (sub-)command via the `--help` flag.

**Listing all packages:** To list all packages built by the server, their version and build status, use:
```shell
# List all managed packages:
serene list
```

**Adding packages:** Serene currently supports two types of packages, those from the AUR, and custom git repositories. The custom git repositories must be formulated the same way as AUR repositories are. An added package is built immediately. Adding them is straight forward:
```shell
# Adding an AUR package called `my-package`.
serene add my-package

# Adding a custom repository from github. The optional devel flag specifies that it is a development package (e.g. it works like -git).
serene add --custom --devel https://github.com/my-user/my-package
```

**Removing packages:** To remove a package, just call the remove subcommand with the package base:
```shell
# Removes `my-package`.
serene remove my-package
```

**Build package now:** To build a package now, run this subcommand with the package base:
```shell
# Builds `my-package` now.
serene build my-package
```

**See package information:** To see all information for a package, you can use the info command and its various subcommands:
```shell
# Get an overview about the package `my-package`. Add `--all` to see all, and not just the latest eight builds.
serene info my-package

# Print the PKGBUILD used for the package currently in the repository to stdout.
serene info my-package pkgbuild

# See more information about the latest build. Supply an id for a specific one.
serene info my-package build

# See the logs of the latest build. Supply an id for a specific one.
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
serene info my-package set prepare "echo 'i am run before the package'"

# Set additional flags which are passed to makepkg when building. See `makepkg --help` for more information. Note that only some options are supported.
serene info my-package set flags "nocheck holdver"
```

**Print the local secret of the CLI:** To print the local secret again, run the following:
```shell
# Prints the secret to add to the `authorized_secrets` file.
serene secret
```

## Building Manually
There are various ways to build and install the cli on your own system. By default, the serene server builds a package for the cli automatically. This package can then be downloaded and installed, either through pacman, or manually. This is the recommended way, but the package can also be installed entirely manually:

### Using the PKGBUILD
A `PKGBUILD` for the cli is provided in this repository. It is used by the serene server when building the package, but can also be used manually. However, since it is not in a normal, empty repository, the workflow would look like this:
```shell
git clone https://github.com/VirtCode/serene-aur
cd serene-aur/cli
makepkg -irsc
```

### Building Directly
You can also build the package directly using cargo. To do that, clone the repository and build it from the **root directory** of the package:
```shell
cargo build --release --bin serene-cli
```
Now, copy the built binary (at `target/release/serene-cli`) to your path, preferably as `serene`, as that is how the package will call the binary.

## Configuration
The cli does not offer much local configuration. It does set up everything needed automatically on the first startup, like prompting the user for the location of the server, generating a secret, etc. This makes it very easy to set up.

This configuration is stored at `~/.config/serene.yml` (or wherever your xdg-config-home is) as YAML. It contains the following attributes:
```yaml
# Local secret used by the cli in plain text.
secret: [my-secret]

# Url of the server that is used.
url: [my-server-url]
```