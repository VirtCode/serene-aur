## Runner
The runner is a docker container which is spun up by the [server](../server/README.md) to actually build a package. 

In essence, it is just an [ArchLinux docker container](https://hub.docker.com/_/archlinux/) with some basic configuration and a shell script. Because it is based on that, it should be frequently updated, and you should always track the *latest* or *main* tag.

It is also important that currently, the server does not pull the image for this container automatically. Thus, you must make sure you have an image pulled. You can do so with this command. Automatic pulling will be implemented in the future, which will make sure that the local image is regularly updated.
```shell
docker pull ghcr.io/virtcode/serene-aur-runner:main
```

The server automatically creates instances of this image, so you don't have to configure anything. You can change the runner image used [on the server](#configuration), but note that it must behave very similarly to the normal one.

### Building
To build the runner container by yourself, you can clone this repository, and build from inside the `runner/` (this) directory. You could run this from the root.
```shell
cd runner
docker build -t serene-aur-runner
```