# Backups

Vaultime backs up your library and history to a folder by itself every day. Cloud backup is an optional extra for invited accounts, encrypted on your PC before anything leaves it.

## Local backups

Local backups are plain folders you can keep anywhere. Settings, Local backups holds everything about them.

Back up automatically
: On from the start. Vaultime makes a backup once a day and when it quits, unless the last one is less than an hour old, and keeps the newest seven. Backups you save by hand are never deleted.

Automatic backups go to
: The `backups` folder inside the data folder to begin with. **Change** picks another one. A folder on another drive, or one that syncs to another device, keeps your history safe when the drive fails.

Save a backup
: Makes a backup right now, into a folder you pick.

Restore from a backup
: Shows what is inside a backup, and only **Replace with this backup** replaces your history with it.

**History** lists the latest backups with their time.

## What a backup holds

A backup holds your games, every session with its full log, the ledger, your settings and the cached artwork.

Some things belong to the PC they were set on:

- The look, the window size and the folder automatic backups go to are in the backup, but a restore keeps this PC's own. On a PC where nobody ever changed the look, like a new one, the restore brings the look of the backup: mode, ground tone, accent, and the dim and blur for a background picture.
- The background picture and the key that signs the PC's ledger are left out of backups.

The ledger notes every restore.

## Restoring

A restore takes in only the sessions that pass their check with the keys this PC trusts, or that this PC vouched for before in the same state. A session of this PC that was still running when the backup was made is closed as [Recovered](trust-labels.md) at its last saved time, counting at most five minutes past the last note the ledger holds. A session another PC was still running stays open as that PC recorded it, see [Several PCs](several-pcs.md#running-sessions). A backup of a PC this one has never seen is taken at its word, like a [first merge](several-pcs.md#which-sessions-count).

On a fresh install with no history of its own, restoring a backup of another PC makes this PC continue as that PC. The restore preview asks first, with a switch that is on by default. Turn it off and this PC stays the PC it is, and the backup's sessions come in as another PC's, as on a PC with history of its own. See [Several PCs](several-pcs.md#setting-a-pc-up-again).

Vaultime refuses a backup that is damaged or holds database parts it never writes.

## Cloud backup

> [!NOTE]
> Cloud backup is a free private beta for people we invite. It may end at any time, and Vaultime works fully without it. You can apply for an invite on the [start page](https://vaultime.codfishcloud.de/#beta).

### Turning it on

Cloud backup is off on a new install, and the menu has no Cloud page. Turn on **Use cloud backup** under Settings, Cloud backup, and the Cloud page joins the menu. Turned off, Vaultime neither signs in nor uploads backups, and it does not read a saved sign-in from the keyring. A PC that is signed in keeps cloud backup until you sign out. PCs that used Vaultime before the switch existed keep cloud backup on while they are signed in.

### Signing up

On the Cloud page, **Create an account** asks for your email, a password, your invite code and a backup passphrase. The password signs you in. The passphrase encrypts your backups and is at least 12 characters long.

> [!CAUTION]
> Keep the backup passphrase safe. Nobody can recover it for you, and without it a cloud backup cannot be restored. Your local backups do not depend on it.

### How it works

- Backups are encrypted on your PC with your passphrase before they are uploaded, so the server only stores data it cannot read.
- Once unlocked, a key made from the passphrase stays in your system's keychain with your sign-in, so daily backups run without asking. The passphrase itself is not kept.
- **Back up every day** uploads a backup of this PC once a day while you are signed in, whatever your other PCs upload. **Back up now** uploads one right away.
- Artwork is stored once per account, so unchanged covers are not uploaded again.
- **Restore** on a backup works like a local restore, after you confirm.
- **Registered PCs** lists every PC that signed in with your account. **Remove** frees the place of a PC you no longer use, and its backups stay. A PC still in use registers again when it next signs in.

### Space and rotation

Every account gets the same space, 1 GiB for backups and artwork, and keeps up to 30 backups. **Space used** on the Cloud page shows how much is taken. When there are 30 backups or the space is full, a new backup replaces the oldest ones once it is safely stored. The newest backup of each PC always stays, so your other PCs can merge it. The server refuses a backup larger than 512 MiB or one that does not fit the space beside the newest backups of your other PCs. It also refuses a new backup of a PC less than 15 minutes after that PC's last one.

There are no paid plans and no tiers. The limits are there to stop abuse.

### What the server sees

Your email address, the name, platform and app version of each PC you back up from, and for each backup when it was made, how large it is and how many games, sessions and images it holds. It also sees the encrypted size of each image, and the public keys each PC signs its [ledger](ledger.md) with, so a merge from the cloud can check them. The name you gave a PC goes with its backups encrypted with your passphrase. The server cannot read your games, sessions, artwork or PC names. The [privacy policy](../../legal/privacy-policy.md) has the details.
