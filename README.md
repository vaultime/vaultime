<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="assets/vaultime-lockup-light.svg">
    <img src="assets/vaultime-lockup-dark.svg" alt="Vaultime" width="380">
  </picture>
</p>

<p align="center">
  <strong>Your game library and a playtime tracker you can trust.</strong><br />
  Local-first. Windows and Linux. Free.
</p>

<p align="center">
  <a href="https://github.com/vaultime/vaultime/actions/workflows/ci.yml"><img src="https://github.com/vaultime/vaultime/actions/workflows/ci.yml/badge.svg" alt="CI"></a>
  <a href="https://github.com/vaultime/vaultime/actions/workflows/release.yml"><img src="https://github.com/vaultime/vaultime/actions/workflows/release.yml/badge.svg" alt="Release"></a>
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-GPL--3.0-7a5ea6" alt="GPL-3.0 license"></a>
  <img src="https://img.shields.io/badge/platform-Windows%20%7C%20Linux-555" alt="Windows and Linux">
</p>

---

Vaultime keeps track of how long you play your PC games, no matter which launcher they come from. It tells apart the time a game was open, the time you actually played and the time it sat idle in the background. Everything stays on your PC, and your library looks like a library, with the artwork from your own game folders.

## Features

- **Automatic tracking.** Once a game is in your library, start it however you like and Vaultime records the session. No launcher integration needed.
- **Real playtime.** Runtime, active playtime and idle time are counted separately.
- **Sleep aware.** Time while your PC sleeps is never counted.
- **Runs in the background.** Closing the window keeps Vaultime tracking in the tray, and it starts when you log in. Both can be turned off in Settings. The tray menu shows the game running and how long you played today.
- **Game discovery.** Reads what Steam, the Xbox app, Epic, GOG, EA, Ubisoft Connect, Battle.net, Riot, Rockstar, Amazon Games, itch.io, HoYoPlay, Legacy Games, Big Fish, Heroic and Lutris have installed, and scans common game folders on every drive for the rest. Any other game can be added by hand.
- **Your artwork.** Steam games get Steam's own covers, others the art from their game folders. Choose the cover per game, frame it, add your own pictures in most image formats, and delete the ones you do not want.
- **Your look.** Dark or light, six ground tones, any accent color and a background picture. The tray and taskbar icon take your accent.
- **A window like a game client.** Fixed sizes from Compact to Extra large, or a free window you can resize.
- **A journal of your play.** Every session written as a sentence, week by week, with an honest trust label and the reason when something looked off.
- **Stats for every year.** A calendar of every day, the hours you play, your games and each month. Point at a day or a month to see what you played, and click a day to read its week in the journal.
- **Game pages in the game's colors.** Its history by day, month and year, every day of the year, totals, and the cover you pick.
- **Always in view.** A live bar shows the running game and its timer, Ctrl+K jumps to any game or page, and Ctrl+F finds a game in your library.
- **Backups.** Automatic local backups every day, restore with a preview, and optional encrypted cloud backup.

## Download

Vaultime is free. Get the latest version from [vaultime.codfishcloud.de](https://vaultime.codfishcloud.de/#download) or from the [releases on GitHub](https://github.com/vaultime/vaultime/releases/latest), which have the same installers. When a new version is out, Vaultime shows it and installs it on one click.

### Windows

Windows 10 and 11 are supported. Download `Vaultime-setup.exe` and run it. No admin rights are needed. The `.msi` package is there for managed installs.

### Linux

| Distribution | Package |
|---|---|
| Debian, Ubuntu, Linux Mint, Pop!_OS | `.deb` |
| Fedora | `.rpm` |
| Arch, openSUSE and any other | `.AppImage` |

For the AppImage, make it executable (`chmod +x Vaultime.AppImage`) and start it.

On X11 Vaultime asks the X server which window is in front and when you last used the keyboard or mouse. There is nothing extra to install. Games that keep the screensaver off still turn idle when you leave. On Wayland the same works for games that run through XWayland, which most games do, Proton games included. GNOME also reports how long you have been away from any window.

Signing in to cloud backup needs a keyring service such as GNOME Keyring or KWallet. Most desktops ship one.

## How playtime is counted

| Time | Meaning |
|---|---|
| Runtime | How long the game was running |
| Active | Runtime while the game was in front and you were at your PC |
| Idle | Runtime while the game sat in the background or you were away |

Playtime from before Vaultime can be imported from Steam in Settings. It counts toward each game's total, minus the time Steam and Vaultime both counted, so nothing counts twice. A session you corrected comes off at the length Vaultime first tracked, so a cut stays cut when you read Steam again. A session you added by hand only comes off when you marked it as played through Steam. Imported time stays out of the journal and the stats, because Steam does not say when you played.

You count as away after 5 minutes without keyboard, mouse or controller input. On Windows, controller input means Xbox controllers and controllers that act as one. On Linux it means every controller. You can change the 5 minutes in Settings.

A game in the background turns idle 15 seconds after it leaves the front, so a quick look at another window does not cost you active time. In Settings you can choose to count background time as active instead.

Time counts on the day it happened. A session from 23:00 to 02:00 counts one hour on the first day and two on the next, in the stats and in the journal's day totals. The journal lists it under the day it started.

## Trust labels

Every session carries a label that tells how much its record can be trusted:

- **Local.** Recorded normally and unchanged since.
- **Suspicious.** The system clock jumped or the record was changed outside Vaultime. A suspicious session stays Suspicious when you correct it.
- **Recovered.** Vaultime was closed while the game ran, for example after a crash, and the session was closed on the next start. It keeps the time up to Vaultime's last note, at most 30 seconds before the crash.
- **Edited.** Tracked, then corrected by you. The old times and your reason stay in the session's log.
- **Manual.** Added by you for play Vaultime did not see, like a session on another PC. It counts as active time.

The library marks a game when one of its sessions from the last 30 days is Suspicious or Recovered, and the game's page counts all of them.

These labels show changes. They cannot prevent them. They are hints, not proof.

To correct a session, point at it in the journal or on the game's page and pick Correct the time. For a game left running, count it only until you stopped playing. For a session that was no play at all, take out all its time. To add play from elsewhere, pick Add a session on the game's page.

## Backups

- **What stays on each PC.** The look, the background picture and the window size belong to the PC they were set on. Backups leave them out, and a restore keeps the ones of the PC it runs on.
- **Local backups** are folders you can keep anywhere. Vaultime makes one by itself once a day and when it quits, and keeps the newest seven. Pick their folder under Settings, Local backups, ideally on another drive or in a folder that syncs. Save a backup makes one by hand, and Restore from a backup shows what is inside before anything is replaced.
- **Cloud backup** is optional, free and invite-only, a private beta that can end at any time. Vaultime works fully without it. Backups are encrypted on your PC with your backup passphrase before they are uploaded, so nobody else can read them. Keep the passphrase safe. Without it, a backup cannot be restored. When you have 30 backups, a new upload replaces the oldest one. When your 1 GiB of storage for backups and artwork is full, delete older backups to make room.

## Your data

| | Windows | Linux |
|---|---|---|
| Library and history | `%APPDATA%\com.vaultime.app` | `~/.local/share/com.vaultime.app` |
| Log files | `%LOCALAPPDATA%\com.vaultime.app\logs` | `~/.local/share/com.vaultime.app/logs` |

Settings, Export saves every finished session as CSV for a spreadsheet or as JSON for other tools. Times are in your time zone and durations in seconds.

Vaultime has no analytics and no telemetry. Details are in the [privacy policy](docs/legal/privacy-policy.md) and the [terms](docs/legal/terms-of-service.md).

## Platform support

| Platform | Status |
|---|---|
| Windows 10 and 11 | Supported |
| Linux on X11 | Supported |
| Linux on Wayland | Works for games that run through XWayland, which most games do, Proton games included. Native Wayland windows and idle time can be misread. |
| macOS | Not available yet |

## License

Vaultime is free software. The app is licensed under the [GNU General Public License](LICENSE), version 3 or later. The cloud server and its install tools are licensed under the [GNU Affero General Public License](apps/api/LICENSE), version 3 or later, so a changed server that others use online has to share its source as well.

The interface components from shadcn/ui are under the [MIT license](LICENSES/MIT.txt), and the fonts Mona Sans, Geist and Geist Mono under the SIL Open Font License 1.1.

The name Vaultime and the logo are not covered by these licenses. A modified version needs its own name and logo.

Contributions come with a short agreement, see [CONTRIBUTING.md](CONTRIBUTING.md).
