# Dependency Resolving
Some AUR packages require dependencies, which themselves are only available on the AUR. To efficiently deal with this problem, the server supports resolving dependencies on adding and before a build.

## Setup
While resolving functionailty is enabled by default, it might not work out of the box on a given setup. To build a package with dependencies from your custom repository, the build container needs to be access it to install dependencies with `pacman` before the build. The serene server will configure `pacman` in the build containers to use such a repository.

To enable this, you'll have to set the `OWN_REPOSITORY_URL` [configuration variable](./readme.md) to an HTTP(S) url with which the **build container can access the server container**. The easiest way is to put in the publicly accessible domain of your server i.e. `OWN_REPOSITORY_URL: https://your-domain/x86_64`. To be a bit more efficient, you can of course also set a local URL or IP with is only valid for the build containers.

_Note that the provided compose files in this documentation already have set this up._

## Principles
Dependency Resolving is not as simple as it sounds and there are a lot of edge cases. This is why Serene has a few strict principles with which dependency resolving is done. While you don't need to be aware of them, it can be helpful to have heard them when you are debugging why some dependency has not been resolved.

**Dependencies will be automatically added when adding a package.** When you are using the [CLI](../usage/cli.md) to add a package, Serene will try to satisfy it's dependencies which are not available in the arch repositories or already in its repository by using packages from the AUR. If a dependency cannot be resolved, the adding will fail. Of course, the [CLI](../usage/cli.md) will report which packages have been added. The important caveat here is that Serene **will NOT automatically add new dependencies if they change after an update**. If this happens, Serene will mark that build as _aborted_.

**Automatically added dependencies are marked as such.** Packages which are added as a dependency will have the `dependency` flag set on them. You can see the flag if it is present using the [CLI](../usage/cli.md) with `serene info my-package`. In the future, it will be possible to remove all orphaned dependencies. You can set and remove this flag for any package as you please with `serene info my-package set dependency true/false`.

**Packages are built in order of their dependence.** Some packages depend on new versions of their dependencies to successfully build. This is why serene tries to build packages in order of dependence ([this can be disabled](./readme.md) with `RESOLVE_BUILD_SEQUENCE`). Note that **this order will only be kept inside a build session**, which means between packages which were scheduled to build at the same time, or packages which were queued at the same time using the [CLI](../usage/cli.md) with `serene build my-package my-dependency`. By default, if the build of a dependency fails in this sequence, its dependents will be _aborted_ ([you can change this](./readme.md) `RESOLVE_IGNORE_FAILED`.).
