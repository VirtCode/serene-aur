# serene
> *serene* is an easy to deploy, self-hosted, AUR build server that aims at replacing your AUR helper.

## About
This project aims to solve some of the inconveniences when using AUR packages. It is often annoying having to **build your AUR packages on every device** when a new version arrives. Existing prebuilt repositories often don't quite do the trick as they **don't have all the software you need**. Current self-hosted solutions are **not flexible enough** and cumbersome to maintain.

This is where *serene* comes in. It is a self-hosted, pre-built AUR repository and build server which is intended to be really flexible and easy to use, to the point of it to be able to replace an AUR helper. It is also easy to set up and features containerized builds. These are the main features:

- **Easy Hosting**: The whole setup is just one docker container, making it easy to host.
- **Powerful CLI**: You can add, build, remove and diagnose the packages you want to build easily from your shell via the CLI.
- **Flexibility**: Customize setup commands, building schedule, etc. on a per-package basis and even use custom non-aur repositories.

Are you ready to host your own? Head to the [deploying](#deploying) section to deploy a server and install the [cli](#installation).

## State
This software is still in its early stages. While I already successfully use it on a daily basis, it is not bug-free and changes are to be expected. I will try to do my best in not introduce *too many* breaking changes.

There are also a couple of features and improvements that still need to be implemented. Refer to the [TODO File](TODO.md) for planned features, tasks and enhancements and don't hesitate to contribute if interested.

## Architecture
Here's a *very* quick word about the architecture of *serene*. As mentioned, it consists of one main container. This container does the following:
- Hosts the API to interact with the server via the CLI. This API will be documented here in the future, in the meantime have a look at the source code.
- Acts as a webserver to serve the built packages as a standard pacman repository.
- Creates sibling containers to build the packages. Usually, a container is created for each package that needs to be built. This ensures proper isolation between the packages. This is also why it needs permissions to access the docker socket.
- Keeps track of, manages and schedules all the added packages and their builds (obviously).

## Deploying
Here is a quick overview of hosting a serene server, based on the main branch. The server is just a single docker container, making it straightforward: 
1. First, **create an empty file** called `authorized_secrets` in your directory. 
2. Set up a reverse proxy for docker (e.g. traefik) to use SSL/TLS. *Not necessary but highly recommended!*
3. Pull the runner image: `docker pull ghcr.io/virtcode/serene-aur-runner:main`
4. Add the following service to your docker compose in the same directory:
```yaml
# docker-compose.yml > services
serene:
  image: ghcr.io/virtcode/serene-aur:main
  volumes:
    - /var/run/docker.sock:/var/run/docker.sock
    - ./authorized_secrets:/app/authorized_secrets
  labels:
    - "your traefik labels here (the server is open on 80)"
```

Now you are done and can start your deployment. Note that the container **requires write access to the docker socket** so that it can spin up containers for building the package. Also see the documentation about the docker image for all options *that does not yet exist.*

Move on to [installing the cli](#installation) and follow the steps there to access your server.

## Installation
To start using your server, you should install the corresponding cli to communicate with the server to download and build packages. You have the following options to install the cli:

- **Build via makepkg**: You can download and build the corresponding [PKGBUILD](https://raw.githubusercontent.com/VirtCode/serene-aur/main/cli/PKGBUILD) manually on your system, and install the package.
- **Download Manually**: Your server will automatically build the cli by default. If you have not yet added the server to your repositories, you can download the package manually by heading to `https://your-host/x86_64` and finding the package called `serene-cli`. Install it with pacman.
- **Add the Repository**: As the cli is built by default, you could already add as a repository to pacman, as seen [below](#only-as-a-repository). Now install `serene-cli` with pacman.
- **Build Manually**: You can also build it completely manually from source.

The CLI is available under `serene`. You can now run it in your terminal, and it will tell you the next steps. It'll walk you through adding the repository to your pacman config and adding your secret to your server.

### Only as a Repository
If you want to use the repository without instructions from the cli, also quite easy. The hosted server can be used as a normal pacman repository, by adding it to `/etc/pacman.conf`:
```ini
[serene]
SigLevel = Optional TrustAll
Server = https://your-host/x86_64
```
*Notice that we do currently not validate signatures, as this is not currently supported.*

## Disclaimer
When hosting a repository with this project, this repository is **your** responsibility! 

This means that it is your job to check `PKGBUILDs` before adding a package to the repository, as building the packages on an isolated environment does **in no way protect you from malware** if you install the package on your system. So make sure you trust the **software and AUR package maintainers** before adding it into the repository. This is especially important as the server will **automatically build new versions** without any actions from your side.

## License
TODO: Include licensing information here!