## Building
To build the docker image for the server, build the Dockerfile with the **parent directory** as context. This ensures that the correct versions from the `Cargo.lock` are used.

Run this in the repository root:
```shell
docker build . -f server/Dockerfile -t serene-aur
```