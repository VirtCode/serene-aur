# Using Host Docker
Here we go over the simplest possible serene deployment, using the host's docker to spin up build containers. This will require you to mount in your host's docker socket into the serene container, which some might consider bad practice and might shy away from. So just be aware of the security implications that come with that. If you want to avoid this and are familiar with docker, consider using the [Docker in Docker Setup](./docker-in-docker.md).

The structure of serene hosted with this compose will look roughly like this:
```
   ┌────────────────────────────────────────────────────────────────────┐
   │ host docker                                                        │
┌──┤                                                                    │
│  │ ┌─────────────┐  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐ │
└──┼─┤             │  │             │  │             │  │             │ │
   │ │ serene-aur  │  │ serene-aur- │  │ serene-aur- │  │ serene-aur- │ │
   │ │             │  │ runner      │  │ runner      │  │ runner      │ │
   │ │             │  │             │  │             │  │             │ │
   │ └──────┬──────┘  └──────┬──────┘  └──────┬──────┘  └──────┬──────┘ │
   │        └ ─ ─ ─ ─ ─ ─ ─ ─┴─ ─ ─ ─ ─ ─ ─ ─ ┴ ─ ─ ─ ─ ─ ─ ─ ─┘        │
   └────────────────────────────────────────────────────────────────────┘

--- docker socket connection to manage containers
- - http connection (for `OWN_REPOSITORY_URL`)
```

When using this compose file, you should change the following:
1. Create the mountpoints you need (this includes `authorized_secrets` for authentication)
2. Add configuration environment variables (see the [Configuration Section](../configuration/readme.md))
3. Configure a reverse proxy (e.g. [traefik](https://github.com/traefik/traefik)) to access the server from the outside

```yaml
services:
  serene:
    image: ghcr.io/virtcode/serene-aur:latest
    restart: unless-stopped

    # we set an explicit container name so our build container can reach it
    container_name: serene

    environment:
      RUST_LOG: serene=info

      # TODO: add your Serene configuration here, e.g.
      # SIGN_KEY_PASSWORD: ...
      # ...

      # our containers will be able to reach the server with this hostname
      OWN_REPOSITORY_URL: http://serene/x86_64

    volumes:
      # here we mount your host docker socket into the container
      - /var/run/docker.sock:/var/run/docker.sock

      # TODO: mount in the required things here, e.g.
      # - ./authorized_secrets:/app/authorized_secrets
      # ...

    volumes:
      # TODO: mount in the required things here, e.g.
      # - ./authorized_secrets:/app/authorized_secrets
      # ...

    labels:
      # TODO: put your traefik labels here, e.g.
      # traefik.enable: true
      # ...
```
