# Usage
This part of the documentation will go over how you can use a set up serene instance as an end user. We assume here that your serene instance is working reliably and things like [dependency resolving](../configuration/dependency-resolving.md) have been set up.

The main thing you will be doing is adding packages, so have a look at:
- How you can [use the CLI](./cli.md) with some examples.
- What different [package sources](./package-sources.md) there are and how to use them.

The rest of this document will go about some other tricks you may want/need to use.

## Debugging Build Failures
From time to time it can happen that some of your packages won't build successfully anymore. Then you'll have to check what the problem is, either fix it or report it to the package maintainer. Or you could wait and hope it will automagically fix itself. Here are a few tips if you decide to debug it.

#### Viewing Build Logs
The first step will be to view build logs. You can view the latest logs of a package with the [CLI](./cli.md) using:
```shell
serene info <my-package> logs
```
The logs will usually tell you what is going wrong and you can now try to fix it. Note that you can also call the above command during a build to get real-time _live_ logs.

#### Performing a Clean Build
Sometimes you get build failures because a previous build has left the container in a weird state which hinders the newer build. Serene will clean (i.e. remove) build container for most packages, except when they are marked as `devel`. This is because depending on the package, you can shave off quite some build time by making use of incremental builds, and `devel` packages are expected to build often.

So if you get such build failures on a package which is not marked `clean`, i.e. by default all `devel` packages, you can perform a one-off clean build by using:
```
serene build --clean <my-package>
```

If this fixes the problem, but the package still breaks frequently because of that, you might also want to consider marking the package as clean to always do clean builds:
```shell
serene info <my-package> set clean true
```

## Packaging Issues
Sometimes, packages on the AUR can be packaged badly and come with many different issues. Serene implements some countermeasures for common mistakes so you can still build these packages, even though they are not technically correct.

#### Broken `.SRCINFO`
One of the most common issues on the AUR is that the maintainer forgot to update the `.SRCINFO` after changing the `PKGBUILD`. Because most AUR helpers, and Serene included, read the `.SRCINFO` to gather information about a package, this can lead to various inconsistencies.

To fix this, you can force Serene to always generate its own `.SRCINFO` for a certain package and discard the one that would be provided by the [source](./package-sources.md). So you can enable the `srcinfo-override`:
```shell
serene info <my-package> set srcinfo-override true
```

#### Prepare Commands
Some packages require some prerequisites to be met on the system it is built. This might mean having added a certain PGP key to the keyring, or simply that the maintainer did not specify all make dependencies correctly. To remedy that, Serene can run various commands before building the package. These commands are called _prepare commands_.

The _prepare commands_ are essentiall a shell script that will be ran on `bash` before each build of the container. Supposing you have a file containing that script, you can set the _prepare commands_ of a package like this:
```shell
serene info <my-package> set prepare "$(cat path/to/script)"
```

Note that inside this script, you have access to root privileges with `sudo`. So if you e.g. need to install a depenency beforehand, you can use `sudo pacman -S <my-dependency> --noconfirm`.
