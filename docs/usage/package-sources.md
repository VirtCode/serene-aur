# Package Sources
At the heart of Serene are its package sources. A package source is essentially where the `PKGBUILD` that is used to build the package is coming from. Each package has a source associated with it which can modified as desired.

Currently, there exist three different sources which are accessible to the user:
- A package that is [from the AUR](#aur-source)
- Using a custom [git repository](#git-source) instead of the AUR
- Supplying a separate custom [raw pkgbuild](#raw-source) file.

If your are wondering what source an underlying package has, you can see the `source:` field from `serene info <my-package>` using the [CLI](./cli.md).

Another important distinction that is to be made are `devel` sources. Each of the above sources can be in `devel` mode. That means that serene will basically treat the package like a `-git` package and check whether the sources for updates, and not just whether the `PKGBUILD` has changed. See the [arch wiki entry](https://wiki.archlinux.org/title/VCS_package_guidelines#VCS_sources) for more information. You can change the `devel` state for a given package using `serene info <my-package> set devel true/false`.

## Source Types
This documentation entry will now go over the different sources and how you can use them with the [CLI](./cli.md). For more information about the specifics of the commands used, refer to the [CLI documentation](./cli.md).

### AUR Source
To add a package from the AUR is very straight forward with the [CLI]. Just use the `add` command without any special arguments using the AUR package name:
```shell
serene add <my-aur-package>
```
The AUR source will automatically set the `devel` state based on whether the package name ends with `-git`.

### Git Source
The git source allows you to use a custom git repository as a package source. The git repository should follow the same rough format, i.e. contain a `PKGBUILD` and a `.SRCINFO` file. The repository can then be added by using:
```shell
serene add --custom https://<my-git-host>/<my-git-repository>
```

If your package is a `devel` package, you'll need to either also pass `--devel` when adding the package, or set the `devel` state after having added it.

It is recommended that custom repositories contain contain their own `.SRCINFO`. Serene however will automatically generate one based off of the `PKGBUILD` when no such file is found. You may just notice that adding the package could take a bit longer than usual.

### Raw Source
The raw source is for using custom, raw `PKGBUILD` files. You can provide the the CLI with a `PKGBUILD` and Serene will build it for you. You can add a `PKGBUILD` like this:
```shell
serene add --pkgbuild --file path/to/PKGBUILD
```

Note that this source obviously will not update if you don't mark it as a `devel` package (either after adding or with `--devel`) because it will never receive a new `PKGBUILD`. To update such a package, you should simply add it again by using the `--replace` flag to replace the previous source.
