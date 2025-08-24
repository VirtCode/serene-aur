# serene-aur
[Setup](https://virtcode.github.io/serene-aur#setup) &ensp; • &ensp; [Documentation](https://virtcode.github.io/serene-aur) &ensp; • &ensp; [Issues](https://github.com/VirtCode/serene-aur/issues) &ensp; • &ensp; [Docker Image](https://github.com/VirtCode/serene-aur/pkgs/container/serene-aur)

> *Serene* is an easy to deploy, self-hosted, AUR build server that aims at replacing your AUR helper.

This project aims to solve some inconveniences when using AUR packages. It is often annoying having to **build your AUR packages on every device** when a new version arrives. Existing prebuilt repositories often don't quite do the trick as they **don't have all the software you need**. Current self-hosted solutions are **not flexible enough** and cumbersome to maintain.

This is where *Serene* comes in. It is a self-hosted package repository and build server which is intended to be really flexible and easy to use, to the point of it being able to replace an AUR helper. It makes heavy use of containerization for easier setup and clean builds. These are the main features:

- **Easy Hosting**: The whole setup is just one docker container, making it easy to host.
- **Powerful CLI**: You can add, build, remove and diagnose the packages you want to build easily from your shell via the CLI.
- **Flexibility**: Customize setup commands, building schedule, etc. on a per-package basis and even use custom non-aur repositories.

Are you ready to host your own? Head to the the basic [setup instructions](https://virtcode.github.io/serene-aur#setup) in the documentation.

## Workflow
This section briefly covers how one uses the system as an end user via the included [CLI](https://virtcode.github.io/serene-aur/usage/cli), so you can get a feel for it.

Add a package from the [AUR](https://aur.archlinux.org) to the repository, so that it is built automatically:
```shell
serene add my-package
```

Watch the package build by having a look at the real-time _live_ logs during the build:
```shell
serene info my-package logs
```

Check some metadata information about the package, as well as past build status:
```shell
serene info my-package
```

And finally, install the package normally via pacman:
```shell
sudo pacman -Sy my-package
```

> Pro tip: This whole procedure can of course also be conveniently all done in one command with `serene add my-package --install`.

## Installation
_Serene_ is a complete build server. Because of that the setup is a bit more involved than a simple AUR helper. This means it consits of three fundamental steps:
1. Deploying the _Serene Server_ on your server
2. Installing the _Serene CLI_ on your host
3. Configuring `pacman` to use _Serene_'s package repository

You can find detailed setup instructions in the documentation.

**[<kbd>&ensp;<br>&ensp;Setup Instructions&ensp;<br>&ensp;</kbd>](https://virtcode.github.io/serene-aur#setup)**

## Documentation
_Serene_ is now finally extensively documented. Example deployments, configuration options, and basic usage tips can all be found over there.  If you find anything missing from the docs, contributions are very welcome!

**[<kbd>&ensp;<br>&ensp;Documentation&ensp;<br>&ensp;</kbd>](https://virtcode.github.io/serene-aur)**

## Disclaimer
When hosting a repository with this project, this repository is **your** responsibility!

This means that it is your job to check `PKGBUILDs` before adding a package to the repository, as building the packages on an isolated environment does **in no way protect you from malware** if you install the package on your system. So make sure you trust the **software and AUR package maintainers** before adding it into the repository. This is especially important as the server will **automatically build new versions** without any actions from your side.

## License
This project is licensed under the MIT License, see the [LICENSE file](LICENSE) for more information.
