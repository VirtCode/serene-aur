## TODO
The project can be devided into the following areas:

#### The Runner
This is a docker container which runs the builds inside of it.
- [X] Decide on Structure of container
- [X] Create Dockerfile
- [X] Test

#### The Service
This contains some parts which are required by the service.
- [ ] Interact with the AUR to check for updates and obtain build files
- [ ] Clone Repositories to build onto volume and manage those
- [ ] Run Runners on demand
- [ ] Retrieve built files from ran containers
- [ ] Serve built files for pacman to use
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
- [ ] Basic commands for adding and removing packages, and querying status

## Future
These things are future tasks and not priority right now:
- [ ] Signing packages

## Notes
- Fakeroot hangs in container for some reason. The current quickfix is adding `--ulimit "nofile=1024:1048576"` when starting the container. See https://github.com/moby/moby/issues/45436 for more infos.