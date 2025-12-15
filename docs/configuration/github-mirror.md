# GitHub Mirror

The AUR has an [experimental GitHub mirror](https://github.com/archlinux/aur) which mirrors the files for all AUR packages. This mirror is [recommended](https://archlinux.org/news/recent-services-outages/) by arch linux to use during downtime of the AUR.

Serene supports (as of `v0.4.3`) using this mirror instead of the official AUR infrastructure for its [AUR sources](../usage/package-sources.md#aur-source), so that updates can still be done during service outages. Additionally this mechanism can be used to work around rate-limiting if your build server is very large. In fact, you can configure serene to not use the official AUR infrastructure at all anymore.

## Package Sources
One part where the GitHub mirror can be used is for [AUR package sources](../usage/package-sources.md#aur-source). This means that Serene will use the GitHub mirror instead of the AUR repositories to obtain the source files for AUR packages. Checks for new versions will also no longer be done over the [aurweb RPC](https://wiki.archlinux.org/title/Aurweb_RPC_interface) and use the mirror instead too. Note that using the mirror for pacakge sources will still provide all functionality.

To activate this, set the `AUR_GITHUB_MIRROR` [configuration variable](./readme.md) to `true`. This will automatically apply to all your currently used [AUR sources](../usage/package-sources.md#aur-source) and will migrate them the next time they are used. Of course you can also disable it again to revert back to using the official AUR infrastructure.

If you are running into rate-limiting issues, changing the package sources to use the mirror should already be sufficient to significantly reduce requests made.

## Dependency Resolving
Another part where the AUR infrastructure is used is during [dependency resolving](./dependency-resolving.md). In particular when adding a new AUR package, Serene will check whether the package exists and also add all required dependencies as a convenience feature.

This is not possible with just the GitHub mirror, which is why this still done even if using the mirror for package sources. Note that this does not really affect rate limiting because it is only done if you add a package.

However, if you don't want to rely on the AUR infrastructure at all, you can set the `AUR_RESOLVE_ADDING` [variable](./readme.md) to `false`. This will make Serene use the GitHub mirror to check if a package exists, and not use the [aurweb RPC](https://wiki.archlinux.org/title/Aurweb_RPC_interface) to automatically add dependencies. Instead, adding packages with missing dependencies will result in an error, so you have to add them manually beforehand.
