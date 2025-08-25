# Deployment
Deployment of Serene is only supported via Docker. This is because Serene itself requires Docker to spin up build containers, so it doesn't really make too much sense deploying it without it. So make sure you have working docker installation on your server. Serene also doesn't do TLS by itself, so you'll need a reverse proxy (e.g. [traefik](https://github.com/traefik/traefik)) to take care of that. Having TLS set up is **strongly recommended**, especially if you don't do [package signing](../configuration/package-signing.md).

The most important thing to consider when hosting serene is that **it requires write access to the docker socket**. This is because serene needs access to a docker instance to spin up its build containers to build the pacakges. The easiest method is to just give it access to your host's docker socket, but you can also use DIND. Giving a container access to your host's docker socket comes with some security implications which you should be aware of.

Here are some examples of `docker-compose.yaml` files for you to start off of. Currently we have two different deployment strategies:
- [Host Docker](./host-docker.md): This deployment uses your host's docker instance to spin up container. Is the shorter and easier one.
- [Docker in Docker](./docker-in-docker.md): This deployment will spin up a separate DIND container for Serene to use for its build containers. Only choose this if you are ready to debug docker and know what you are doing. This is generally recommended though as it is more self-contained and probably also slightly more secure.

After choosing your compose template, read the sections below to set up different things which might be useful.

## Authentication
To add packages to the repository, users using the [CLI](../usage/cli.md) must be authenticated. By default, the whole API that the CLI uses to find out package information is gated behind authentication. Optinally, you can make the read-only parts of the API open for everyone using the `ALLOW_READS` [configuration variable](../configuration/readme.md). This can be useful if other people want to use your repository and want to see how the packages are built.

The secrets for the API and CLI are read from the `authorized_secrets` file that is found in the container under `/app`. Make sure **you mount such a file into your container by adding e.g. `- ./authorized_secrets:/app/authorized_secrets` to your volumes**. The structure of that file is very similar to the authorized keys file of SSH. An example file would look like this:
```
34dw1hPxSEiEAjYJluPCmxPE50NKjzO35E1Spi8b2iU= me@laptop
+RS1uMC0L/CA2P0JfbSrI4CwoaXu+Hw0m7uCXyB09yg= me@desktop
```

The **[CLI](../usage/cli.md)** will generate such a line for a secret that it will use on the first startup and print it out. You should add that line verbatim to your `authorized_secrets` file on your server. You can use `serene secret` to print this line again after the first startup.

> [!NOTE]
> If you want to use the API manually, you'll want to generate such a line yourself.
> - The first word per line contains the `BASE64` encoded `SHA256` hash of the trusted secret. The rest of the line (after the space) is not relevant. By default it will contain the user- and hostname the secret belongs to.
> - To now authenticate with the API, the client must provide the secret in the `Authorization`-header without anything else, in plain text. The server will match against the hashes and allow requests if it is included.

## Versioning
It is heavily recommended that you use the release version of Serene. This means using the docker image either tagged with a version `vX.X.X` or `latest` which will point to the latest version. When you are using a tagged version, Serene will ensure that relevant components are in sync:
- It will use a runner image that is compatible with the current version.
- The [CLI](./usage/cli.md) that the server builds will also match the version of the server (you can change this with the `EDGE_CLI` [config variable](../configuration/readme.md)).

Make sure you **update your instance timely when a new release comes out**. This is because GitHub will only build new runner images for the latest release. While this should not directly lead to any problems, you might have a lot of overhead because each build container first needs to be updated to the latest arch linux packages.

If you are feeling adventurous you can also use the `main` branch. For that, use a docker image tagged with `main`. Note that updates between these versions might not go smoothly without manual intervention, so be careful and know what you are doing. Note that when using such an image, the [CLI](./usage/cli.md) will by default also be built from the `main` branch.

## Container Content
The actual Serene server container is as lightweight as possible and based on alpine. Many things are obviously stored on the container's filesystem. Based on your setup, you might want to mount some things outside of the container and potentially onto other volumes.

These files should be mounted in if you want to use the corresponding functionality:
- `/app/authorized_secrets`: This **file** contains the secrets which are allowed to access the api. You probably want to mount this outside the container for easier access.
- `/app/sign_key.asc`: This file contains the private key used for package signing if provided.

Internally, the container uses the following locations to store its stuff:
- `/app/serene.db`: This is the *sqlite* database where all the builds, logs, etc. are stored about the different packages.
- `/app/sources`: This is a directory structure that stores the `PKGBUILD`s which are copied to containers for building.
- `/app/repository`: This is the repository containing the built packages. It is served as is for pacman to access.
- `/app/logs`: This is the directory which contains the build logs for all packages.

### Backups
It can be a good practice to back up your serene instance because when using it for long, you will have modified `PKGBUILD`s and different package-specific changes which are not available for download on the AUR. In the case you loose your server, you want to be able to restore a serene instance quickly.

It is recommended to backup the following two locations:
- `/app/serene.db`: The db contains your package-specific changes and potential custom `PKGBUILD`s.
- `/app/sources`: You'll need to back this up too, as serene cannot recrate this folder based off of a database if it looses it. It should be relatively small as it only contains the actual `PKGBUILD`s it has downloaded.

Note that we _don't_ backup the built packages stored in `/app/repository`. This is because there are the biggest files, and these packages should be able to be rebuilt easily (try `serene build --all --force`). If you don't want to rebuild tho, you can consider backing them up too at your own storage cost.

## Container Endpoints
By default, the docker image will open it's webserver on port `80`, so make sure you configure your reverse proxy to use that. In general, the webserver can be thought of consisting of two parts:
- The package repository is located at `/[arch]` so most probably on `/x86_64` and does not require authentication. The repository follows the format of an ordinary Arch Linux repository, just the way pacman expects it. The name of the actual repository (determined by the .db file) can be set with the `NAME` [configuration variable](../configuration/readme.md), and is `serene` by default. The package archive format is `.tar.zst`.
- The REST API the [CLI](../usage/cli.md) uses is located at `/package`. This api can be used to query, add, etc. packages. To use it, [authentication](#authentication) is required. The endpoints of the API will be documented in the future™. In the meantime, have a look at the [endpoints](https://github.com/VirtCode/serene-aur/tree/main/server/src/web/mod.rs) and the used [data structs](https://github.com/VirtCode/serene-aur/tree/main/server/data/src) in the source code of the server. For a reference implementation, you may have a look at the [CLI's code](https://github.com/VirtCode/serene-aur/tree/main/cli/src/web/requests.rs).
