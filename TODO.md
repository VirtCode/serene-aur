## TODO
adjusted for v0.3.0 on 23.02.24

#### Polish 
*polish for a better user experience*
- [ ] Add more debug and info logs
- [X] Make tables of cli adaptive to text width
- [ ] Disclaimer about pkgbuilds
- [ ] Clear versioning
- [X] Shell completions with dynamic packages

#### Cleanness
*changes for a cleaner overall process*
- [X] Enable multilib in build container
- [X] Set makepkg.conf to build to /app/target (to avoid conflicts with any packages)
- [X] Figure something out to avoid code duplication from server to cli
- [X] Cache more data inside the sources after upgrading, so that we don't have to read the filesystem as often
- [ ] Create aur-specific normal source, so that we can check for updating without git
- [ ] Combine devel and non-devel git sources 
- [X] Be able to view pkgbuild from cli
- [ ] More efficient queries for the web handlers

#### Improvements
*not too heavy improvement which can be made*
- [X] Removal of packages from the server
- [ ] `serene check` command to compare output of `pacman -Qm`
- [X] allow changing of settings for package, e.g. enable, schedule, clean
- [X] allow inspection of builds through cli
- [ ] on-boarding screen when first using the cli, with config to configure pacman, validate server connection
- [X] Build cli by default on server
- [ ] Pull runner image automatically on startup and periodically
- [ ] Rebuild cleaned when non-clean containers fail
- [X] Add itself as a source to build container, so we have rudimentary aur dependency support

#### Must haves
- [X] Store state in a database and not a json file
- [ ] Signing packages
- [X] Local / Custom source, where a user can upload a custom pkgbuild

#### Features
*features which are kinda important*
- [X] Pre-launch scripts to configure container specifically for package (e.g. with `eww`'s keys)
- [X] Readme and Documentation
- [X] License
- [X] CI and ghcr

## Future
*things that would be nice but are absolutely not priority*
- [ ] Handle in-aur dependencies
- [ ] Allow attachment at build process to view logs real-time
- [ ] Web frontend to view package status