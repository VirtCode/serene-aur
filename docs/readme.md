# Serene Documentation
Welcome to the documentation for Serene. This page contains most of what you need to know to run and maintain an instance of serene yourself. If you find anything here incomplete, don't hesitate to contribute!

The documentation is split into roughly three parts:
- [Usage](./usage/readme.md): This covers how you use serene as an end user to build and maintain your packages.
- [Configuration](./configuration/readme.md): This section is about how you can configure the _server_ side of serene to support various things.
- [Deployment](./deployment/readme.md): This part is for deploying serene yourself with docker, and contains example compose files.

This file here will go about the general structure of serene and a quick setup guide.

## Architecture
Serene makes heavy use of docker to build it's package and to provide some isolation between different builds. This means you'll need to host serene on a server with docker, and it is recommended to use the docker image that we provide to do that.

The structure of a conventional server looks about like this (excluding deployment-specific details):
```
 Your Server:                                              Your Host:

      ┌─────────────────────────────────┐                   ┌────────────┐
      │                                 │                   │            │
      │     Serene Server Container     ├─ ─ ─ ─ ─ ─ ─┬─ ─ ─┤ Serene CLI │
      │                                 │                   │            │
      └─────┬──────────┬──────────┬─────┘             │     └────────────┘
        ┌───┘          │          └───┐
  ┌─────┴─────┐  ┌─────┴─────┐  ┌─────┴─────┐         │     ┌────────────┐
  │           │  │           │  │           │               │            │
  │ Build     │  │ Build     │  │ Build     │         └─ ─ ─┤   Pacman   │
  │ Container │  │ Container │  │ Container │               │            │
  │           │  │           │  │           │               └────────────┘
  └───────────┘  └───────────┘  └───────────┘
```

As you can see, serene consists of three fundamental parts:
- **Serene Server** (serene-aur): This is the heart of serene. It is the main container you will have to deploy. It creates and coordinates different build containers and offers a rest api for the cli to interact with. It also doubles as a webserver to serve the different packages ready for pacman to use.
- **Build Container** (serene-aur-runner): The server will automatically create instances of this container to build packages. A container is created for each package that is build and will be removed afterwards.
- **Serene CLI** (serene-cli): This is a CLI you can run on your host machine to interact with the server. You can use it to add, inspect, and rebuild packages.

## Setup
Based on the architecture above, we can see that setting up serene consists of three fundamental steps:
1. Deploying the _Serene Server_ on your server
2. Installing the _Serene CLI_ on your host
3. Configuring `pacman` to use Serene's package repository
This will now briefly cover what each step entails and link you to the relevant documentation.

#### 1. Deploying Serene
To host the _Serene Server_ you'll need a server which is reasonably powerful (while not required, I recommend at least 2 cores and 8gb of memory, and at least 50gb of free storage). Also note that the server should have the **same architecture as your end host**, which for you probably means you'll need an `x86_64` server. If you don't own a server, a relatively cheap VPS will usually do the trick, just don't expect fast build times.

You'll need the following prerequisites before deploying Serene:
- A working docker installation.
- A reverse proxy for docker (e.g. [traefik](https://github.com/traefik/traefik)) and a domain name to set up TLS.

Now you are ready to deploy serene. To spin up _Build Containers_ it will require write access to the docker socket. There are different ways to achieve this and we have example `docker compose` setups for multiple methods:
- [Using the host's Docker](./deployment/host-docker.md): This is the easiest and shortest method to set up Serene, recommended for new users.
- [Using Docker in Docker](./deployment/docker-in-docker.md): If you cringe at the idea of giving Serene access to your docker socket, DIND is probably the way to go for you.

After choosing a deployment, make sure you read through the [Deployment Section](./deployment/readme.md) to set up things like **authentication** amongst other things. After that, head on to the [Configuration Section](./configuration/readme.md) to configure your server for your usecase.

After setup, check that your server is running by heading to `https://your-domain`. You should see basic version information there if everything worked.

#### 2. Installing the CLI
After having successfully set up a server, you probably want to install the CLI to communicate with the server to add and build packages. You'll only need to do this step once on your host, as afterwards your own build server will build the CLI automatically (which can take some time) and you can install it via pacman. With your server running for a while, run:

```
curl -LSs https://your-domain/x86_64/package/serene-cli -o /tmp/serene-cli && sudo pacman -U /tmp/serene-cli
```

The CLI is now installed and should be available as `serene`. Run `serene --help` to check that your installation is working. Future updates will be taken care of by pacman as you'll now add your repository. You can run any other command and it will walk you through your next steps. For more usage, head to the [CLI Section](./usage/cli.md).

Alternatively, you can use one of the following methods to install the cli:
- **Build on your host**: You can download and build and install the [PKGBUILD](https://github.com/VirtCode/serene-aur/tree/main/cli/PKGBUILD) manually.
- **Download Manually**: Your server will automatically build the CLI by default. You can quickly retrieve it using the redirect at `https://your-domain/[arch]/package/serene-cli`.
- **Add the repository**: As the cli is built by default, you could already add as a repository to pacman, as seen [below](#3.-configuring-pacman). Now install `serene-cli` via pacman.
- **Build manually**: Of course you can build the CLI from source manually too.

#### 3. Configuring Pacman
The last step is to configure pacman to use your new repository. **This can be done via the CLI as you have now installed it.** It will walk you through the whole process and will also help setting up [package signing](./configuration/package-signing.md). So it is recommended to use that.

You can also set it up without the CLI tho, by adding the following to `/etc/pacman.conf`. *The SigLevel should only be set to `Never` when [package signing](./configuration/package-signing.md) is disabled for the repository. Consider setting it up.*:

```ini
[serene]
SigLevel = Never
Server = https://your-domain/x86_64
```
