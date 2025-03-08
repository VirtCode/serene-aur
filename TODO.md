## TODO

#### Bugs

*unintended behaviour that should be fixed*

- [ ] When using OWN_REPOSITORY on a new instance, without one previously built package, the containers can't udpate themselves

#### Polish

*polish for a better user experience*

- [ ] Add more debug and info logs
- [X] Make tables of cli adaptive to text width
- [ ] Disclaimer about pkgbuilds
- [X] Clear versioning
- [X] Shell completions with dynamic packages
- [ ] show webhooks feature in infos
- [X] fix cli PKGBUILD so version doesn't have a v- prefix
- [ ] Wait for build on multiple packages
- [ ] Add troubleshooting section to docs (e.g. for when build lock is set but failed beyond fatally)
- [X] clap binary name is off in help cmd

#### Cleanness

*changes for a cleaner overall process*

- [X] Enable multilib in build container
- [X] Set makepkg.conf to build to /app/target (to avoid conflicts with any packages)
- [X] Figure something out to avoid code duplication from server to cli
- [X] Cache more data inside the sources after upgrading, so that we don't have to read the filesystem as often
- [ ] Create aur-specific normal source, so that we can check for updating without git
- [ ] Combine devel and non-devel git sources
- [ ] Show info about source (type, some url) on `serene info`
- [ ] Move SRCINFO gen to container because the current impl is veery sketchy
- [X] Be able to view pkgbuild from cli
- [ ] More efficient queries for the web handlers
- [ ] Combine DB and Broadcast to one "storage" object

#### Improvements

*not too heavy improvement which can be made*

- [X] Removal of packages from the server
- [ ] `serene check` command to compare output of `pacman -Qm`
- [X] allow changing of settings for package, e.g. enable, schedule, clean
- [X] allow inspection of builds through cli
- [ ] on-boarding screen when first using the cli, with config to configure pacman, validate server connection
- [X] Build cli by default on server
- [X] Pull runner image automatically on startup and periodically
- [ ] Rebuild cleaned when non-clean containers fail
- [X] Add itself as a source to build container, so we have rudimentary aur dependency support
- [ ] Import server public key through cli
- [ ] add `serene manage key` to retrieve the server key
- [X] Add build reason to build struct (https://github.com/VirtCode/serene-aur/issues/10)
- [ ] Purge logs after a certain age
- [ ] Create `manage purge` command which removes all deps that are no longer needed
- [ ] Add a `srcinfo-gen` setting to packages which will tell serene to generate a srcinfo before each build to fix bad packages, see #21

#### Must haves

- [X] Store state in a database and not a json file
- [x] Signing packages
- [X] Local / Custom source, where a user can upload a custom pkgbuild

#### Features

*features which are kinda important*

- [X] Pre-launch scripts to configure container specifically for package (e.g. with `eww`'s keys)
- [X] Readme and Documentation
- [X] License
- [X] CI and ghcr

## Future

*things that would be nice but are absolutely not priority*

- [X] Handle in-aur dependencies
- [X] Allow attachment at build process to view logs real-time
- [ ] Web frontend to view package status - **out of scope, can easily be achieved as an external thing**
- [ ] Support other vcs than git for devel packages

## Roadmap
- [X] v0.4.0 - Dependency Resolving
- [ ] v0.4.1 - Cleaner sources, less aur git usage
