# Using Docker In Docker
This contains an example compose file to host Serene with Docker in Docker. The structure will then look somewhat like this:
```
┌───────────────────────────────────────────────────────────────────────┐
│ host docker                                                           │
│                                                                       │
│ ┌────────────┐  ┌───────────────────────────────────────────────────┐ │
│ │            │  │ docker in docker                                  │ │
│ │            ├──┤                                                   │ │
│ │            │  │ ┌─────────────┐  ┌─────────────┐  ┌─────────────┐ │ │
│ │ serene-aur │  │ │             │  │             │  │             │ │ │
│ │            │  │ │ serene-aur- │  │ serene-aur- │  │ serene-aur- │ │ │
│ │            │  │ │ runner      │  │ runner      │  │ runner      │ │ │
│ │            │  │ │             │  │             │  │             │ │ │
│ │            │  │ └──────┬──────┘  └──────┬──────┘  └──────┬──────┘ │ │
│ └─────┬──────┘  └────────┼────────────────┼────────────────┼────────┘ │
│       └─ ─ ─ ─ ─ ─ ─ ─ ─ ┴ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ┘          │
└───────────────────────────────────────────────────────────────────────┘

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
    image: ghcr.io/virtcode/serene-aur:main
    restart: unless-stopped
    depends_on:
      docker:
        condition: service_healthy

    environment:
      RUST_LOG: serene=info

      # TODO: add your Serene configuration here
      # SIGN_KEY_PASSWORD: ...
      # ...

      # because we have configured a predefined subnet, our build containers can reach
      # the server under this IP
      OWN_REPOSITORY_URL: http://10.10.10.10/x86_64

      # we tell Serene to use docker from the DIND container via plain tcp
      DOCKER_URL: tcp://docker:2375

    volumes:
      # TODO: mount in the required things here, e.g.
      # - ./authorized_secrets:/app/authorized_secrets
      # ...

    labels:
      # TODO: put your traefik labels here, e.g.
      # traefik.enable: true
      # ...

    networks:
      docker:
        ipv4_address: 10.10.10.10

  docker:
    image: docker:dind
    restart: unless-stopped

    # you might want to put some limits here to constrain resouces used by the build containers
    # cpus: 3.5
    # mem_limit: 7g

    privileged: true
    # we currently disable tls because setting up local certificates would be a pain and unnecessary
    entrypoint: dockerd --host tcp://0.0.0.0:2375 --tls=false

    environment:
      DOCKER_HOST: tcp://localhost
    healthcheck:
      test: ["CMD", "docker", "version"]
      start_period: 30s
      start_interval: 1s

    volumes:
      # if you want to store your container data somewhere specific, create a mount point here
      # - ./big-container-files:/var/lib/docker
    networks:
      docker:

networks:
  docker:
    # we have to fix a subnet and give the container a fixed ip to allow the dind containers to access it reliably
    # sadly, DNS is not possible, see https://github.com/docker-library/docker/issues/133
    ipam:
      config:
        - subnet: 10.10.0.0/16
```
