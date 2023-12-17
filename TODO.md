## TODO
The project can be devided into the following areas:

#### General
- [ ] Add more debug statements

#### The Runner
This is a docker container which runs the builds inside of it.
- [X] Decide on Structure of container
- [X] Create Dockerfile
- [X] Test

#### The Service
This contains some parts which are required by the service.
- [X] Interact with the AUR to check for updates and obtain build files
- [X] Clone Repositories to build onto volume and manage those
- [X] Run Runners on demand
- [X] Retrieve built files from ran containers
- [X] Serve built files for pacman to use
- [ ] Schedule Builds

#### The Database
The database which is used to store what packages are installed etc.
- [ ] Store current packages with their url and other stuff
- [ ] Store build logs for each build
- [ ] Get that working, probably with diesel

#### The API
The api is a part of the service which handles user interaction.
- [ ] Handle user logon with a secret from known_secrets file.
- [ ] Create Endpoints for adding and removing a package, getting status (probably with actix)

#### The CLI
The cli is on the client and should interact with the api
- [ ] Generate secret on first start and prompt user
- [ ] Basic commands for adding and removing packages, and querying status

## Future
These things are future tasks and not priority right now:
- [ ] Signing packages
- [ ] Handle in-aur dependencies
- [ ] Allow pinning a package to a specific commit
- [ ] Store state in a database and not a json file
- [ ] Dependency resolving on AUR
- [ ] Set makepkg config in runner

## Notes
- Fakeroot hangs in container for some reason. The current quickfix is adding `--ulimit "nofile=1024:1048576"` when starting the container. See https://github.com/moby/moby/issues/45436 for more infos.