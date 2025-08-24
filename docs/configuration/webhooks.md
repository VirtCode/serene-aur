# Webhooks
Serene supports receiving webhooks to trigger builds for packages. This means you can for example trigger a rebuild of your own software every time you push to your git repository.

## Setup
Webhooks are disabled by default. To enable webhooks, you will need to set the `WEBHOOK_SECRET` [configuration variable](./readme.md) to a secret (we call this the _server secret_). The _server secret_ can be a random string but does not have to be terribly secure. You can change the _server secret_ to invalidate all already created _webhook secrets_.

## Using Webhooks
If you have enabled webhooks you can now proceed to create a _secret_ for a specific package. This _webhook secret_ will only work to trigger webhooks for the package it is being created for. Note that _webhook secrets_ are stateless and are **bound to the user that has created it**. So if this user is removed from the `authorized_secrets` their _webhook secrets_ will no longer work. To create a webhook secret, you can use the [CLI](../usage/cli.md):
```
serene manage webhook <my-package>
```

This command will print the _webhook secret_ for that given package along with a URL. To trigger the webhook, you can send an http `POST` request to the URL that is provided. It will be of the form:
```
https://your-domain//webhook/package/<my-package>/build?secret=<webhook-secret>
```

#### Example: GitHub
To use webhooks to build a package every time somebody pushes to a GitHub repository you control, you can use GitHub's integrated webhook feature. Note that you have to have admin access to the repository, so it's really only useful for your own software.

Go to your repository, then `Settings` > `Webhooks` then `Add Webhook`. Then:
- Enter the URL you have received from the [CLI](../usage/cli.md) into `Payload URL`.
- Select `application/json` from `Content type`.
- **Leave the secret field empty.**
- Leave SSL verification enabled.

Then click on `Add Webhook` and you are ready to go.
