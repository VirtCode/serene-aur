# serene-aur
> *serene* is an easy to deploy, self-hosted, AUR build server that aims at replacing your AUR helper.

This project aims to solve some inconveniences when using AUR packages. It is often annoying having to **build your AUR packages on every device** when a new version arrives. Existing prebuilt repositories often don't quite do the trick as they **don't have all the software you need**. Current self-hosted solutions are **not flexible enough** and cumbersome to maintain.

This is where *serene* comes in. It is a self-hosted package repository and build server which is intended to be really flexible and easy to use, to the point of it being able to replace an AUR helper. It makes heavy use of containerization for easier setup and clean builds. These are the main features:

- **Easy Hosting**: The whole setup is just one docker container, making it easy to host.
- **Powerful CLI**: You can add, build, remove and diagnose the packages you want to build easily from your shell via the CLI.
- **Flexibility**: Customize setup commands, building schedule, etc. on a per-package basis and even use custom non-aur repositories.

Are you ready to host your own? Head to the [deploying](#deploying) section to deploy a server and install the [cli](#installation), then look at some basic [usage](#usage).

## Usage
This section briefly covers how one uses the system as an end user, via the included cli. See [installation](#installation) for information about how to deploy the server and install the CLI locally.

Add a package from the [AUR](https://aur.archlinux.org) to the repository, so that it is built automatically:
```shell
serene add my-package
```

List all currently managed packages, their version and build status:
```shell
serene list
```

Get more specific information about a package as well as the past builds:
```shell
serene info my-package
```

Setup commands to run before building the package:
```shell
serene info my-package set prepare "add some keys && do something else"
```

Many more commands are found on the documentation for the CLI:

**[<kbd>&ensp;<br>&ensp;CLI Documentation&ensp;<br>&ensp;</kbd>](./cli/README.md)**

## State
This project is still in its early stages, but already usable on a daily basis. There are also a couple of improvements and optimizations that still need to be implemented.
Big, recently implemented features include:
- [X] Entirely custom PKGBUILDs without a git repository
- [X] Package signing
- [X] Automatic AUR dependency resolving

Refer to the [TODO File](TODO.md) for more features, tasks and enhancements and don't hesitate to contribute if interested.

## Installation
Installing serene involves two things, deploying the server, and installing a local CLI to conveniently interact with the server.

### Deploying
Here is a quick overview of hosting a serene server, based on the main branch. The server is just a single docker container, making it straightforward:
1. First, **create an empty file** called `authorized_secrets` in your directory.
2. Set up a reverse proxy for docker (e.g. traefik) to use SSL/TLS.
3. Add the following service to your docker compose in the same directory:
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

Now you are done and can start your deployment. Note that the container **requires write access to the docker socket** so that it can spin up containers for building the package.

##### Next Steps
You have now deployed a basic server with default settings successfully. You are now ready to start building your first packages on your new server. To configure more advanced features like **automatic dependency resolving** or **package signing**, see the full server documentation:


**[<kbd>&ensp;<br>&ensp;Server Documentation&ensp;<br>&ensp;</kbd>](./server/README.md)** &ensp; **[<kbd>&ensp;<br>&ensp;Runner Documentation&ensp;<br>&ensp;</kbd>](./runner/README.md)**

### Installing the CLI
To start using your server, you should install the corresponding cli to communicate with the server to download and build packages. You have the following options to install the cli:

- **Build via makepkg**: You can download and build the corresponding [PKGBUILD](cli/PKGBUILD) manually on your system, and install the package.
- **Download Manually**: Your server will automatically build the cli by default. If you have not yet added the server to your repositories, you can download the package manually by heading to `https://your-host/x86_64` and finding the package called `serene-cli`. Install it with pacman.
- **Add the Repository**: As the cli is built by default, you could already add as a repository to pacman, as seen [below](#installing-only-the-repository). Now install `serene-cli` with pacman.
- **Build Manually**: You can also build it completely manually from source.

The CLI is available under `serene`. You can now run it in your terminal, and it will tell you the next steps. It'll walk you through adding the repository to your pacman config and adding your secret to your server. For all the other features, see the full documentation:

**[<kbd>&ensp;<br>&ensp;CLI Documentation&ensp;<br>&ensp;</kbd>](./cli/README.md)**

### Installing only the Repository
If you want to use the repository without instructions from the cli, also quite easy. The hosted server can be used as a normal pacman repository, by adding it to `/etc/pacman.conf`:
```ini
[serene]
SigLevel = Optional TrustAll
Server = https://your-host/x86_64
```
*The SigLevel should only be set to `Optional TrustAll` when [package signing](./server/README.md#package-signing) is disabled for the repository*

## Architecture
Here's a *very* quick word about the architecture of *serene*:
- **Server Container:** API and file Server for the repository. Manages all the packages and schedules.
- **Runner Container:** Spun up by the Server Container as a sibling container on the host. Build only one package each.
- **Local CLI:** Interacts with said API to add and manipulate added packages. Requires authentication via secret.

## Disclaimer
When hosting a repository with this project, this repository is **your** responsibility!

This means that it is your job to check `PKGBUILDs` before adding a package to the repository, as building the packages on an isolated environment does **in no way protect you from malware** if you install the package on your system. So make sure you trust the **software and AUR package maintainers** before adding it into the repository. This is especially important as the server will **automatically build new versions** without any actions from your side.

## License
This project is licensed under the MIT License, see the [LICENSE file](LICENSE) for more information.