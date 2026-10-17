# Your data and privacy

Everything Vaultime records stays in one folder on your PC. It has no analytics and no telemetry, and it only goes online to look for updates and, if you use it, for cloud backup.

## Where your data lives

| | Windows | Linux |
|---|---|---|
| Library and history | `%APPDATA%\com.vaultime.app` | `~/.local/share/com.vaultime.app` |
| Log files | `%LOCALAPPDATA%\com.vaultime.app\logs` | `~/.local/share/com.vaultime.app/logs` |

Inside the data folder:

`vaultime.db`
: The database with your games, sessions, their logs, the ledger and your settings.

`asset-cache`
: Copies of your cover art and other images.

`backups`
: Automatic backups, unless you picked another folder.

`appearance`
: The background picture, if you set one.

`device-id`, `device-key`, `device-keys`
: This PC's random id, the key that signs its ledger and the list of keys it has signed with. See [The ledger and integrity](ledger.md).

Leave these files alone while Vaultime runs. Editing the database by hand is exactly what the [trust labels](trust-labels.md) are there to notice.

## Export your sessions

Settings, Export saves every finished session:

- **Export as CSV** for a spreadsheet,
- **Export as JSON** for other tools. The file names its format, `vaultime-sessions`, and a version.

Times are in your time zone and durations in seconds.

## What goes online

Updates
: Vaultime asks the Vaultime server whether a new version is out when it starts and every few hours. Updates are signed, and Vaultime only installs ones that carry the right signature.

Cloud backup
: Only when you sign in. Backups are encrypted on your PC before they are uploaded, see [Backups](backups.md#cloud-backup).

Nothing else
: No analytics, no telemetry, no crash reports and no ads. Covers come from Steam's files on your PC and from your game folders, not from the internet.

The [privacy policy](../../legal/privacy-policy.md) and the [terms](../../legal/terms-of-service.md) have the full details.

## What Vaultime reads on your PC

To find games and tell active from idle time, Vaultime reads which programs run, which window is in front and when you last used the keyboard, mouse or a controller. It keeps only that input happened, never which key or button. To find games it reads the install lists of your launchers, and for the Steam import it reads Steam's playtime file.

## Remove your data

Delete the data folder to remove your library and history from this PC. Backups you saved elsewhere are not touched. To delete a cloud account with all its backups, write to the address in the privacy policy.
