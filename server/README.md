# Server
The server is the main part of serene, as it actually builds and manages the packages. It spins up other containers, called [runners](../runner/README.md) to actually build the packages.

The server is distributed in the form of a docker container. This file documents the direct usage, building and configuration of it.

## Repository
The server will expose a package repository over http. It is located at `/[arch]` so most probably on `/x86_64` and does not require authentication. 

The repository follows the format of an ordinary Arch Linux repository, just the way pacman expects it. The name of the actual repository (determined by the .db file) can be [configured](#configuration) on the container, and is `serene` by default. The package archive format is `.tar.zst`.

## API
Additionally, the server exposes a REST API under`/packages`. This api is used by the CLI to interact with the server to query, add, etc. packages. By default, the whole API is secured behind authentication via a secret. Read-only endpoints can be [configured](#configuration) to be open for everyone.

The secrets for the API are read from the `authorized_secrets` file that is [mounted into the container](#deployment). It is very similar in structure to the authorized keys file of SSH. An example file would look like this:
```
34dw1hPxSEiEAjYJluPCmxPE50NKjzO35E1Spi8b2iU= me@laptop
+RS1uMC0L/CA2P0JfbSrI4CwoaXu+Hw0m7uCXyB09yg= me@desktop
```

The first word per line contains the `BASE64` encoded `SHA256` hash of the trusted secret. The rest of the line (after the space) is not relevant. By default it will contain the user- and hostname the secret belongs to. To now authenticate with the API, the client must provide the secret in the `Authorization`-header without anything else, in plain text. The server will match against the hashes and allow requests if it is included.

The endpoints of the API will be documented in the future. In the meantime, have a look at the [endpoints](src/web/mod.rs) and the used [data structs](data/src) in the source code of the server. For a reference implementation, you may have a look at the [CLI's code](../cli/src/web/requests.rs).

## Container
The container is as lightweight as possible and thus alpine-based. Any configuration is done through environment variables on the container.

### Structure
The most important thing to know about the structure of the server is to know that it builds the packages in separate containers. This means that it needs to be able to spin up sibling containers, which requires **access to the docker socket**. See in the deployment section.

It also stores many things on the filesystem, which may or may not be of interest to you. Here are the major paths that are used inside the container:
- `/app/authorized_secrets`: This **file** contains the secrets which are allowed to access the api. You probably want to mount this outside the container for easier access.
- `/app/serene.db`: This is the *sqlite* database where all the builds, logs, etc. are stored about the different packages.
- `/app/sources`: This is a directory structure that stores the `PKGBUILD`s which are copied to containers for building.
- `/app/repository`: This is the repository containing the build packages. It is served as is for pacman to access.

### Deployment
It is recommended to deploy the container via docker compose. Here is an example for a basic deployment:
```yml
# docker-compose.yml

version: "3.8"

services:
  serene:
    image: ghcr.io/virtcode/serene-aur:main
    restart: unless-stopped
    environment:
      RUST_LOG: info
    volumes:
      - /var/run/docker.sock:/var/run/docker.sock
      - ./authorized_secrets:/app/authorized_secrets
    labels:
      - "traefik.configuration.here.please"

  # ... other services, like traefik
```

There's a couple of things to note here:
- We can set the configuration for the container over the `environment` section. see [below](#configuration) for all the options.
- We mount the **file** `authorized_secrets` into the container. This will contain the secrets that are allowed to access the api.
- We mount the docker socket into the container. **This is necessary** as serene has to be able to spin up runner containers.
- We (or more *you*), configure traefik as a load balancer. This is because it is **strongly recommended** to use https to download packages and connect to the api, especially if the packages are not signed.

You can also create more bind mounts to access the built packages from the file system, or also configure more things through environment variables.

### Configuration
The server features a few configuration options, which should be changed when creating the container. The default values are given below, options which are not intended for normal customization are marked with `DEBUG`.
```shell
# name of the repository that is created (i.e. serene.db.tar.gz)
# this must be the same as the category in the /etc/pacman.conf
NAME=serene

# whether the serene-cli is added and built automatically
BUILD_CLI=true

# http url to use to reference own package store from the outside
# if this url is provided, packages can use dependencies of others inside the container
OWN_REPOSITORY_URL=none

# default schedules for checking and building the packages
# for devel packages (i.e. -git) a separate option exists, it is the normal one if unset
SCHEDULE=0 0 0 * * *
SCHEDULE_DEVEL=0 0 0 * * *

# sets the log level for the container, recommended to set to info
RUST_LOG=none

# sets the docker image that is used for creating build containers
# note that these images should behave the same way the normal one does
RUNNER_IMAGE=ghcr.io/virtcode/serene-aur-runner:main

# name prefix for runner containers (names will be [prefix][package-name])
RUNNER_PREFIX=serene-aur-runner-

# do not require authentication for the read-only parts of the api
ALLOW_READS=false

# DEBUG the port the server binds to in the container
PORT=80

# DEBUG the architecture that is targeted
ARCH=[architecture of server]
```

### Building
To build the server container by yourself, you can also clone this repository. Build it directly from the **root** directory with docker. You could run this from the root.
```shell
docker build . -f server/Dockerfile -t serene-aur
```
