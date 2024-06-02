## Runner
The runner is a docker container which is spun up by the [server](../server/README.md) to actually build a package. 

In essence, it is just an [ArchLinux docker container](https://hub.docker.com/_/archlinux/) with some basic configuration and a shell script. 
The docker image is built by a [Actions job](https://github.com/VirtCode/serene-aur/actions/workflows/publish-runner.yml) every night, to include the newest dependencies and changes from ArchLinux.

Because ArchLinux updates very frequently, the server will automatically pull the latest image every night (and on startup).

Containers of this image will be created automatically by the server, so you don't have to configure anything. You can change the runner image used [on the server](#configuration), but note that it must behave very similarly to the normal one.

### Building
To build the runner container by yourself, you can clone this repository, and build from inside the `runner/` (this) directory. You could run this from the root.
```shell
cd runner
docker build -t serene-aur-runner
```