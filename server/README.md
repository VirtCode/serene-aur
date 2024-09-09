# Server
The server is the main part of serene, as it actually builds and manages the packages. It spins up other containers, called [runners](../runner/README.md) to actually build the packages.

The server is distributed in the form of a docker container. This file documents the direct usage, building and configuration of it.

## Repository
The server will expose a package repository over http. It is located at `/[arch]` so most probably on `/x86_64` and does not require authentication.

The repository follows the format of an ordinary Arch Linux repository, just the way pacman expects it. The name of the actual repository (determined by the .db file) can be [configured](#configuration) on the container, and is `serene` by default. The package archive format is `.tar.zst`.

### Package Signing
For security reasons the packages built by the build server can be signed. Whilst the signature is no guarantee the built package does not contain any malware it still verifies the package was built on your build server.
This is useful when you host your repository without a ssl certificate (which is generally discouraged) or when there are mirrors which redistribute packages from your build server.

To enable package signing you'll have to mount a gpg private key into the server container. While testing the package signing we found **RSA keys** have to have a minimum length of **2048 bits** to work with the library used for signing.

The gpg private key can be exported like this:
```shell
# export private key into `private-key.asc` in armored ascii format
gpg --armor --output private-key.asc --export-secret-key <key-id>
```

>[!IMPORTANT]
> It's highly recommended to use a new gpg key for the package signing which isn't used anywhere else. Putting a private key and it's password onto a publicly accessible server poses a risk for identity theft!

Now, when [deploying](#deployment) you'll have to add an additional volume bind to the `docker-compose.yml`:
```yml
# docker-compose.yml

services:
  serene:
    # ... remaining service definition
    volumes:
      - /path/to/private-key.asc:/app/sign_key.asc
      # ... other volumes

```
If the signing is enabled after the initial setup of the server only new package builds will be signed. Old packages need to be rebuilt in order to get signed.

Should your private key be protected by a password you'll have to additionally [configure the password](#configuration).

To enable package verification using pacman you'll need to download the public key from the `/key` api endpoint and [import it into your pacman keys](https://wiki.archlinux.org/title/Pacman/Package_signing#Adding_unofficial_keys).

Since the packages are now signed you can remove the `SigLevel = Optional TrustAll` from the [repository definition](../README.md#installing-only-the-repository) in the `pacman.conf`.
By removing this configuration pacman will fall back to the `SigLevel` defined at the `[options]` level which by default only allows signed packages to be downloaded from a repository.
More about the `SigLevel` configuration can be read in the [arch wiki](https://wiki.archlinux.org/title/Pacman/Package_signing#Configuring_pacman).

>[!NOTE]
> The downloading and importing of the public key should in the future be possible through the cli

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
- `/app/repository`: This is the repository containing the build packages. It is served as is for pacman to access.
- `/app/sign_key.asc`: This **file** contains the private key used for package signing if provided

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
RUNNER_IMAGE=ghcr.io/virtcode/serene-aur-runner:main

# schedule for pulling the latest runner image
SCHEDULE_IMAGE=0 0 0 * * *

# name prefix for runner containers (names will be [prefix][package-name])
RUNNER_PREFIX=serene-aur-runner-

# do not require authentication for the read-only parts of the api
ALLOW_READS=false

# string used for signing webhook secrets
# when left empty, webhooks are disabled
WEBHOOK_SECRET=none

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
