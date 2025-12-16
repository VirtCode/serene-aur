# Configuration
To use Serene effectively for your own needs, you should configure it. Most configuration is done using environment variables so it is easy to use with docker.

There are a couple of important features which require some configuration to work.
- Enabling proper [dependency resolving](./dependency-resolving.md) (recommended).
- Making use of [package signing](./package-signing.md).
- Using [webhooks](./webhooks.md) for automation.
- Switch to the [GitHub mirror](./github-mirror.md) in case of AUR outages.

The rest are small tweaks which are relatively self-explanatory and are documented below. Note that the options involved in the above features are documented here again too.

## Options
The following options are available for customization. The default values are given below.
```shell
# name of the repository that is created (i.e. serene.db.tar.gz)
# this must be the same as the category in the /etc/pacman.conf
NAME=serene

# whether the serene-cli is added and built automatically
BUILD_CLI=true

# whether the serene-cli should be built from the `main` branch instead of
# a known tag compatible with the server
EDGE_CLI=false

# http url to use to access its own repository
# this is used by the runner containers to access the repository for dependencies
# it is easiest to set this to the url the repo is accessible with from the outside (e.g. https://my.tld/x86_64)
OWN_REPOSITORY_URL=none

# optional password to unlock the private key used for package signing
SIGN_KEY_PASSWORD=none

# use the experimental github mirror instead to get aur package sources
# this can be used to still add and update packages if the AUR is inaccessible
AUR_GITHUB_MIRROR=false

# use the aur to add and resolve dependencies when adding a package
# otherwise missing dependencies will result in an error
AUR_RESOLVE_ADDING=true

# whether to set newly added packages to build automatically
# this corresponds to the default value of the "enable" setting in the cli
SCHEDULING_DEFAULT=true

# default schedules for checking and building the packages
# for devel packages (i.e. -git) a separate option exists, it is the normal one if unset
SCHEDULE=0 0 0 * * *
SCHEDULE_DEVEL=0 0 0 * * *

# sets the log level for the container, recommended to set to info
RUST_LOG=none

# sets the docker image that is used for creating build containers
# note that these images should behave the same way the normal one does
# automatic versioning will only be done if the image tag contains {version}
# the server will try to pull docker images with its tag as that version
RUNNER_IMAGE=ghcr.io/virtcode/serene-aur-runner:edge-{version}

# prune all old images when updating the runner image
# warning: prunes all unused images known to the used docker instance
PRUNE_IMAGES=true

# schedule for pulling the latest runner image
SCHEDULE_IMAGE=0 0 0 * * *

# name prefix for runner containers (names will be [prefix][package-name])
RUNNER_PREFIX=serene-aur-runner-

# do not require authentication for the read-only parts of the api
ALLOW_READS=false

# string used for signing webhook secrets
# when left empty, webhooks are disabled
WEBHOOK_SECRET=none

# mirror used to synchronize package databases
# must contain {repo} and {arch}, with will be filled with the corresponding repo and architecture
SYNC_MIRROR=https://mirror.init7.net/archlinux/{repo}/os/{arch}

# build the package in order of the dependency tree
# if true, dependencies between packages are resolved before building, so they are built in the correct order
RESOLVE_BUILD_SEQUENCE=true

# ignore build failures of dependencies while building
# if false, a package build will be aborted if dependencies fail to build
RESOLVE_IGNORE_FAILED=false

# maximal amount of packages which can build concurrently
# this is a limit on a per-session basis, i.e. per schedule target or manual trigger
CONCURRENT_BUILDS=5
```

## Advanced Options
The following options are more advanced, only use them if you know what you are doing.
```shell
# the unix or tcp url to docker with a prefix (e.g. tcp://127.0.0.1:2375)
# the runner containers will be spun up on this docker instance
DOCKER_URL=unix:///var/run/docker.sock

# the port the server binds to in the container
PORT=80

# the architecture that is targeted
ARCH=[architecture of server]

# disable the package scheduling, so packages won't build automatically
SCHEDULING_DISABLED=false

# timeout (in milliseconds) that is used to connect to the AUR RPC
# and how many retries are made per request
AUR_REQUEST_TIMEOUT=5000
AUR_REQUEST_RETRIES=1
```
