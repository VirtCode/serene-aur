# Package Signing
For security reasons the packages built by the build server can be signed. Whilst the signature is no guarantee the built package does not contain any malware it still verifies the package was built on your build server. This is useful when you host your repository without a TLS certificate (which is generally discouraged) or when there are mirrors which redistribute packages from your build server.

To enable package signing you'll have to provide the server container with a private key. A suitable key must have a minimum length of 2048 bits. There are two methods to provide a private key to your server, either via keyfile, or via a running [gpg-agent](https://www.gnupg.org/(en)/documentation/manuals/gnupg24/gpg-agent.1.html). It's **highly recommended to use a new gpg key for the package signing which isn't used anywhere else**. Putting a private key and it's password onto a publicly accessible server poses a risk of identity theft!

Also note that if the signing is enabled after the initial setup of the server **only new package builds will be signed. Old packages need to be rebuilt in order to get signed.**

## Using the signatures in Pacman
If you have successfully set up package signing, you'll still have to make `pacman` verify it.

When you are setting up a new host, the onboarding procedure of the [CLI](../usage/cli.md) will walk you through setting up signatures automatically, if they are enabled on your server.

If you are already using Serene on a host, you can also use the [CLI](../usage/cli.md) to guide you through importing the server's key on your machine. Make sure you **don't forget to also update the `SigLevel`** in your pacman config.

However, it can also easily be done manually done in three simple steps:
1. Obtain the public key. You can do this via `serene manage key` if you have the [CLI](../usage/cli.md) installed, or you can download it directly from the `/key` api endpoint.
2. Import the key into your `pacman` keyring. See the corresponding [arch wiki section](https://wiki.archlinux.org/title/Pacman/Package_signing#Adding_unofficial_keys).
3. Tighten the `SigLevel` in your pacman config. Remove the line `SigLevel = Optional TrustAll` from the pacman config if you have added it during [setup](../readme.md#3.-configuring-pacman). See the [arch wiki section](https://wiki.archlinux.org/title/Pacman/Package_signing#Configuring_pacman) for more information about it.

## Setup with a Keyfile
The easiest way to configure signing is by providing the container with a private keyfile. When not using any hardware devices to provide the secret key, this is the recommended setup.

A gpg private key can be exported like this (again, make sure to have a seperate key just for Serene):
```shell
# export private key into `private-key.asc` in armored ascii format
gpg --armor --output private-key.asc --export-secret-key <key-id>
```

Now, when [deploying](../deployment/readme.md) you'll have to add another bind mount to the `docker-compose.yml`:
```yml
# docker-compose.yml

services:
  serene:
    # ... remaining service definition
    volumes:
      - /path/to/private-key.asc:/app/sign_key.asc
      # ... other volumes
    environment:
      # if your key requires a password, add the following to your envs
      - SIGN_KEY_PASSWORD: <key-password>
      # ... remaining config
```

## Setup with `gpg-agent`
If you want to store your private key in a more secure way, for example in a hardware device, you can use the `gpg-agent` as your keystore and let the server use the key from there.

To use this, you'll need to have a `gpg-agent` running and accessible to the server. The agent will then be exposed to the server inside the container via a bind mount to its socket. It is recommended to start your gpg-agent via your init system or something similar.

<details>
<summary><b>Managing the agent with systemd</b></summary>

Create a systemd socket and service for your gpg-agent:
```ini
# /etc/systemd/system/serene-gpg-agent.socket

[Unit]
Description=GnuPG cryptographic agent and passphrase cache for serene

[Socket]
ListenStream=/etc/serene/gnupg/S.gpg-agent
FileDescriptorName=std
SocketMode=0600
DirectoryMode=0700

[Install]
RequiredBy=docker.service
```
```ini
# /etc/systemd/system/serene-gpg-agent.service

[Unit]
Description=GnuPG cryptographic agent and passphrase cache for serene
Requires=serene-gpg-agent.socket

[Service]
ExecStart=/usr/bin/gpg-agent --homedir /etc/serene/gnupg --supervised
ExecReload=/usr/bin/gpgconf --homedir /etc/serene/gnupg --reload gpg-agent
```

Enable the service and test whether your devices are accessible:
```bash
# enable the socket
sudo systemctl enable serene-gpg-agent.socket

# test if your hardware key is detected
sudo GNUPGHOME=/etc/serene/gnupg gpg-connect-agent "LEARN --sendinfo" /bye
```
</details>

The `gpg-agent` only handles the secret key and the actual signing process. You still need to provide the server with the public key that matches the private key you intend to use. The server will then search for the corresponding private key in the agent. If there are multiple suitable keypairs, the first one found will be used.

>[!NOTE]
> If using a PIN-protected hardware device, the server will refuse to use if only one PIN attempt remains. This is to prevent accidental lockouts or key destruction. If you encounter this, perform some action that requires a PIN (e.g., sign something manually with GPG) to reset the PIN retry counter or reset it using the admin PIN.

Now, when [deploying](../deployment/readme.md) you'll have to mount the socket and public key in the `docker-compose.yml`. We assume the `gpg-agent` socket is in `/etc/serene/gnupg/S.gpg-agent`:
```yml
# docker-compose.yml

services:
  serene:
    # ... remaining service definition
    volumes:
      - /path/to/public-key.asc:/app/sign_key.asc
      - /etc/serene/gnupg/S.gpg-agent:/app/S.gpg-agent
      # ... other volumes
    environment:
      # if your key requires a pin/password, add the following to your envs
      - SIGN_KEY_PASSWORD: <key-password>
      # ... remaining config
```
