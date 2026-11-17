# Your own server

Cloud backup runs on a Vaultime server that you set up, for yourself or for you and your friends. It runs on a rented server or on a NAS at home, anywhere Docker runs, on x86 and 64-bit ARM machines alike. Nobody runs a server for you, and everyone sets one up the same way.

> [!NOTE]
> Cloud backup is optional. Vaultime works fully without it, and a [sync folder](backups.md#sync-folder) keeps your backups on your other PCs without any server.

## What you need

- A machine that runs Docker with Docker Compose, such as a small rented Linux server or a NAS.
- An address that reaches it over https, such as `vaultime.example.com`. Vaultime talks to servers over https only, and over plain http only to a server on the same PC.

## Set it up

1. Download [compose.yaml](https://github.com/vaultime/vaultime/blob/main/deploy/server/compose.yaml) and [.env.example](https://github.com/vaultime/vaultime/blob/main/deploy/server/.env.example) into a folder of their own on the machine.
2. Copy `.env.example` to `.env`. Set `VAULTIME_DOMAIN` to the address, and fill in each of the three secrets with a random value of its own, made with `openssl rand -hex 32`.
3. Start the server in one of the two ways below.
4. In Vaultime, turn on **Use cloud backup** under Settings, Cloud backup. On the Cloud page, type the address under **Your server**, pick **Connect** and create your account.

### On a machine of its own

With ports 80 and 443 free and the address pointing at the machine, run `docker compose --profile caddy up -d`. Caddy answers in front of Vaultime and gets a certificate for the address by itself.

### Behind a reverse proxy

Behind a reverse proxy you already run, such as the one of your NAS, run `docker compose up -d` and point the proxy at `http://127.0.0.1:8080`. `VAULTIME_PORT` in `.env` picks another port. On a NAS, its container app takes the two files as a project, and its reverse proxy with a certificate gives the server its https address.

## Accounts for friends

While `VAULTIME_OPEN_SIGNUPS` is `true`, anyone who reaches the server can create an account. Once you and your friends have one, set it to `false` and run `docker compose up -d` again. The Cloud page then says that the server takes no new accounts, and everyone who has one keeps signing in. Open it again whenever a friend joins.

Each account encrypts its backups with its own passphrase. You cannot read your friends' backups, and they cannot read yours.

To lock an account, run this in the folder of the server. Set it back to `active` to let the account in again.

```
docker compose exec db psql -U vaultime -c "UPDATE cloud_accounts SET access_state = 'locked' WHERE email = 'friend@example.com'"
```

## Limits

Every account gets the same limits, so that no account can fill the server. Add a line to `.env` to change one, then run `docker compose up -d` again.

| Setting | Default |
| --- | --- |
| `VAULTIME_MAX_ACCOUNT_BYTES` | 1 GiB for the backups and artwork of an account, `1073741824` |
| `VAULTIME_MAX_COMPLETE_BACKUPS_PER_ACCOUNT` | 30 backups, past which the oldest make room |
| `VAULTIME_MAX_BACKUP_BYTES` | 512 MiB for one backup, `536870912` |
| `VAULTIME_MIN_BACKUP_INTERVAL_SECONDS` | 15 minutes between two backups of a PC, `900` |

## Updates

`docker compose pull` and then `docker compose up -d` bring the newest server. It updates its database by itself when it starts. To stay on one version, set `VAULTIME_VERSION` in `.env`.

## Backups of the server

Everything on the server comes from the PCs. Each PC keeps its whole history and uploads a new backup every day, so a lost server costs only the accounts: create them again and each PC uploads anew. To keep a copy of the server anyway, stop it with `docker compose stop` and copy the Docker volumes `vaultime_db` and `vaultime_backups`.

The server does not run as root. The volumes in `compose.yaml` need nothing, but a folder of your own mounted at `/data` instead has to belong to user and group 10001, for example with `chown -R 10001:10001` on it, or every upload fails.

## What the server sees

The server and whoever runs it see what [Backups](backups.md#what-the-server-sees) lists: email addresses, the PCs of each account and the size and counts of each backup, never games, sessions or artwork.
