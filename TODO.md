## TODO
redone for v0.2.0 on 30.12.23

#### Polish 
*polish for a better user experience*
- [ ] Add more debug and info logs
- [ ] Make tables of cli adaptive to text width
- [ ] Disclaimer about pkgbuilds
- [ ] Clear versioning

#### Cleanness
*changes for a cleaner overall process*
- [X] Enable multilib in build container
- [X] Set makepkg.conf to build to /app/target (to avoid conflicts with any packages)
- [X] Figure something out to avoid code duplication from server to cli
- [X] Cache more data inside the sources after upgrading, so that we don't have to read the filesystem as often
- [ ] Create aur-specific normal source, so that we can check for updating without git

#### Improvements
*not too heavy improvement which can be made*
- [X] Removal of packages from the server
- [ ] `serene check` command to compare output of `pacman -Qm`
- [X] allow changing of settings for package, e.g. enable, schedule, clean
- [X] allow inspection of builds through cli
- [ ] on-boarding screen when first using the cli, with config to configure pacman

#### Features
*features which are kinda important*
- [ ] Signing packages
- [X] Pre-launch scripts to configure container specifically for package (e.g. with `eww`'s keys)
- [ ] Readme & License
- [X] CI and ghcr

## Future
*things that would be nice but are not priority*
- [ ] Handle in-aur dependencies
- [ ] Store state in a database and not a json file
- [ ] Allow attachment at build process to view logs real-time
- [ ] Web frontend to view package status