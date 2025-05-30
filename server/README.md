# Server
The server is the main part of serene, as it actually builds and manages the packages. It spins up other containers, called [runners](../runner/README.md) to actually build the packages.

The server is distributed in the form of a docker container. This file documents the direct usage, building and configuration of it.

## Features
These are the main features the server has which you might want to enable and configure further. For a complete list of configuration options, see the [configuration section](#configuration) below.

### Repository
The server will expose a package repository over http. It is located at `/[arch]` so most probably on `/x86_64` and does not require authentication.

The repository follows the format of an ordinary Arch Linux repository, just the way pacman expects it. The name of the actual repository (determined by the .db file) can be [configured](#configuration) with `NAME`, and is `serene` by default. The package archive format is `.tar.zst`.

### Dependency Resolving
Some AUR packages require dependencies, which themselves are only available on the AUR. To efficiently deal with this problem, the server supports resolving dependencies on adding and before a build.

Resolving is enabled by default. To use it effectively however, you need to give the [runner](../runner/README.md) containers access to your package repository. This has to be done via the `OWN_REPOSITORY` [configuration option](#configuration). Other options to change the resolving process are also available.

When adding a package, its dependencies from the AUR will automatically be resolved and added alongside it. If a dependency cannot be resolved, the adding will fail. The [CLI](../cli/README.md#usage) will report all additionally added packages. Packages which are added this way by the server are automatically marked with a **dependency** which can however be altered after the fact.

During a build, the server will by default also resolve the dependency relationship between all packages that are going to be built (this can be [disabled](#configuration) with `RESOLVE_BUILD_SEQUENCE`). If the dependencies for a package are not satisfied, the package build will be **aborted** prematurely. After the initial resolving, the packages will then be built in order of the dependency tree. If the build of a package fails, later to-be-built packages depending on it will then by default be **aborted** (this can be [changed](#configuration) with `RESOLVE_IGNORE_FAILED`).

Note that the server **will not** add new dependencies, if they change after a package has been added. This is deliberate, such that you don't suddenly find packages on your server you have never added yourself.

### Package Signing
For security reasons the packages built by the build server can be signed. Whilst the signature is no guarantee the built package does not contain any malware it still verifies the package was built on your build server.
This is useful when you host your repository without a ssl certificate (which is generally discouraged) or when there are mirrors which redistribute packages from your build server.

To enable package signing you'll have to provide the server container with a private key. A suitable key must have a minimum length of 2048 bits. There are two methods to provide a private key to your server, either via keyfile, or via a running [gpg-agent](https://www.gnupg.org/(en)/documentation/manuals/gnupg24/gpg-agent.1.html).

>[!IMPORTANT]
> It's highly recommended to use a new gpg key for the package signing which isn't used anywhere else. Putting a private key and it's password onto a publicly accessible server poses a risk for identity theft!

Also note that if the signing is enabled after the initial setup of the server only new package builds will be signed. Old packages need to be rebuilt in order to get signed.

#### Using a Keyfile
The easiest way to configure signing is by providing the container with a private keyfile. When not using any hardware devices to provide the secret key, this is the recommended setup. A gpg private key can be exported like this:
```shell
# export private key into `private-key.asc` in armored ascii format
gpg --armor --output private-key.asc --export-secret-key <key-id>
```

Now, when [deploying](#deployment) you'll have to add another bind mount to the `docker-compose.yml`:
```yml
# docker-compose.yml

services:
  serene:
    # ... remaining service definition
    volumes:
      - /path/to/private-key.asc:/app/sign_key.asc
      # ... other volumes
    environment:
      # if your key requires a password, add the following to your envs
      - SIGN_KEY_PASSWORD: <key-password>
      # ... remaining config
```

#### Using `gpg-agent`
If you want to store your private key in a more secure way, for example in a hardware device, you can use the `gpg-agent` as your keystore and let the server use the key from there.

To use this, you'll need to have a `gpg-agent` running and accessible to the server. The agent will then be exposed to the server inside the container via a bind mount to its socket. It is recommended to start your gpg-agent via your init system or something similar.

<details>
<summary><b>Managing the agent with systemd</b></summary>

Create a systemd socket and service for your gpg-agent:
```ini
# /etc/systemd/system/serene-gpg-agent.socket

[Unit]
Description=GnuPG cryptographic agent and passphrase cache for serene

[Socket]
ListenStream=/etc/serene/gnupg/S.gpg-agent
FileDescriptorName=std
SocketMode=0600
DirectoryMode=0700

[Install]
RequiredBy=docker.service
```
```ini
# /etc/systemd/system/serene-gpg-agent.service

[Unit]
Description=GnuPG cryptographic agent and passphrase cache for serene
Requires=serene-gpg-agent.socket

[Service]
ExecStart=/usr/bin/gpg-agent --homedir /etc/serene/gnupg --supervised
ExecReload=/usr/bin/gpgconf --homedir /etc/serene/gnupg --reload gpg-agent
```

Enable the service and test whether your devices are accessible:
```bash
# enable the socket
sudo systemctl enable serene-gpg-agent.socket

# test if your hardware key is detected
sudo GNUPGHOME=/etc/serene/gnupg gpg-connect-agent "LEARN --sendinfo" /bye
```
</details>

The `gpg-agent` only handles the secret key and the actual signing process. You still need to provide the server with the public key that matches the private key you intend to use. The server will then search for the corresponding private key in the agent. If there are multiple suitable keypairs, the first one found will be used.

>[!NOTE]
> If using a PIN-protected hardware device, the server will refuse to use if only one PIN attempt remains. This is to prevent accidental lockouts or key destruction. If you encounter this, perform some action that requires a PIN (e.g., sign something manually with GPG) to reset the PIN retry counter or reset it using the admin PIN.

Now, when [deploying](#deployment) you'll have to mount the socket and public key in the `docker-compose.yml`. We assume the `gpg-agent` socket is in `/etc/serene/gnupg/S.gpg-agent`:
```yml
# docker-compose.yml

services:
  serene:
    # ... remaining service definition
    volumes:
      - /path/to/public-key.asc:/app/sign_key.asc
      - /etc/serene/gnupg/S.gpg-agent:/app/S.gpg-agent
      # ... other volumes
    environment:
      # if your key requires a pin/password, add the following to your envs
      - SIGN_KEY_PASSWORD: <key-password>
      # ... remaining config
```

#### Using the signatures in Pacman

To enable package verification using pacman you'll need to download the public key. You can do this via `serene manage key` if you have the cli installed, or you can download it directly from the `/key` api endpoint. After that, [import it into your pacman keys](https://wiki.archlinux.org/title/Pacman/Package_signing#Adding_unofficial_keys).

Since the packages are now signed you can remove the `SigLevel = Optional TrustAll` from the [repository definition](../README.md#installing-only-the-repository) in the `pacman.conf`.
By removing this configuration pacman will fall back to the `SigLevel` defined at the `[options]` level which by default only allows signed packages to be downloaded from a repository.
More about the `SigLevel` configuration can be read in the [arch wiki](https://wiki.archlinux.org/title/Pacman/Package_signing#Configuring_pacman).

## API
Additionally, the server exposes a REST API under`/package`. This api is used by the CLI to interact with the server to query, add, etc. packages. By default, the whole API is secured behind authentication via a secret. Read-only endpoints can be [configured](#configuration) to be open for everyone.

The secrets for the API are read from the `authorized_secrets` file that is [mounted into the container](#deployment). It is very similar in structure to the authorized keys file of SSH. An example file would look like this:
```
34dw1hPxSEiEAjYJluPCmxPE50NKjzO35E1Spi8b2iU= me@laptop
+RS1uMC0L/CA2P0JfbSrI4CwoaXu+Hw0m7uCXyB09yg= me@desktop
```

The first word per line contains the `BASE64` encoded `SHA256` hash of the trusted secret. The rest of the line (after the space) is not relevant. By default it will contain the user- and hostname the secret belongs to. To now authenticate with the API, the client must provide the secret in the `Authorization`-header without anything else, in plain text. The server will match against the hashes and allow requests if it is included.

The endpoints of the API will be documented in the future. In the meantime, have a look at the [endpoints](src/web/mod.rs) and the used [data structs](data/src) in the source code of the server. For a reference implementation, you may have a look at the [CLI's code](../cli/src/web/requests.rs).

### Webhooks

The server allows for webhook functionality to trigger builds for packages using a http request. Webhooks require authentication using a special secret. Those secrets are bound to the
authorized secret of a user and a package name. This allows a single webhook secret to only trigger webhooks for one single package. Webhooks are disabled by default, to enable them you need to set the `WEBHOOK_SECRET` [config option](#configuration).

For every user with an authorized secret it's possible to request a webhook token for a specific package using the [CLI](../cli/README.md#usage) or the `/webhook/package/{name}/secret` endpoint where the authorized secret is passed through the `Authorization` header.
The returned secret can be used in the `/webhook/package/{name}/build?secret=<webhook-secret>` HTTP-POST request to trigger a build for the specified package.

The webhook secrets are stateless and bound to an authorized secret of a user. This means every webhook secret is valid as long as the secret of the user is still part of the `authorized_secrets` file. As soon as the user secret is removed from
the server the webhook secret is invalidated as well.

## Container
The container is as lightweight as possible and thus alpine-based. Any configuration is done through environment variables on the container.

### Structure
The most important thing to know about the structure of the server is to know that it builds the packages in separate containers. This means that it needs to be able to spin up sibling containers, which requires **access to the docker socket**. See in the deployment section.

It also stores many things on the filesystem, which may or may not be of interest to you. Here are the major paths that are used inside the container:
- `/app/authorized_secrets`: This **file** contains the secrets which are allowed to access the api. You probably want to mount this outside the container for easier access.
- `/app/serene.db`: This is the *sqlite* database where all the builds, logs, etc. are stored about the different packages.
- `/app/sources`: This is a directory structure that stores the `PKGBUILD`s which are copied to containers for building.
- `/app/repository`: This is the repository containing the built packages. It is served as is for pacman to access.
- `/app/sign_key.asc`: This file contains the private key used for package signing if provided

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

# http url to use to access its own repository
# this is used by the runner containers to access the repository for dependencies
# it is easiest to set this to the url the repo is accessible with from the outside (e.g. https://my.tld/x86_64)
OWN_REPOSITORY_URL=none

# optional password to unlock the private key used for package signing
SIGN_KEY_PASSWORD=none

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

# DEBUG the unix or tcp url to docker with a prefix (e.g. tcp://127.0.0.1:2375)
#       the runner containers will be spun up on this docker instance
DOCKER_URL=unix:///var/run/docker.sock

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
