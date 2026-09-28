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
  <a href="https://github.com/schwimmbeck/vaultime/actions/workflows/ci.yml"><img src="https://github.com/schwimmbeck/vaultime/actions/workflows/ci.yml/badge.svg" alt="CI"></a>
  <a href="https://github.com/schwimmbeck/vaultime/actions/workflows/release.yml"><img src="https://github.com/schwimmbeck/vaultime/actions/workflows/release.yml/badge.svg" alt="Release"></a>
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-MIT-7a5ea6" alt="MIT license"></a>
  <img src="https://img.shields.io/badge/Tauri-2-24c8db" alt="Tauri 2">
  <img src="https://img.shields.io/badge/Rust-1.98-b7410e" alt="Rust 1.98">
  <img src="https://img.shields.io/badge/React-19-61dafb" alt="React 19">
  <img src="https://img.shields.io/badge/platform-Windows%20%7C%20Linux-555" alt="Windows and Linux">
</p>

---

Vaultime keeps track of how long you really play your PC games, no matter which launcher they come from. It tells apart the time a game was open, the time you actually played and the time it sat idle in the background. Everything stays on your computer, and your library looks like a library, with the artwork from your own game folders.

## Features

- **Automatic tracking.** Start a game and Vaultime records the session. No launcher integration needed.
- **Real playtime.** Runtime, active playtime and idle time are counted separately.
- **Sleep aware.** Time while your computer sleeps is never counted.
- **Runs in the background.** Closing the window keeps Vaultime tracking in the tray, and it starts when you log in. Both can be turned off in Settings.
- **Game discovery.** Reads what Steam, the Xbox app, Epic, GOG, EA, Ubisoft Connect, Battle.net, Riot, Rockstar, Amazon Games, itch.io, HoYoPlay, Legacy Games, Big Fish, Heroic and Lutris have installed, and scans common game folders on every drive for the rest.
- **Your artwork.** Steam games get Steam's own covers, others the art from their game folders, and you choose the cover per game.
- **A journal of your play.** Every session written as a sentence, week by week, with an honest trust label and the reason when something looked off.
- **Game pages in the game's colors.** The last two weeks at a glance, totals, and the cover you pick.
- **Always in view.** A live bar shows the running game and its timer, and Ctrl+K jumps to any game or page.
- **Backups.** Automatic local backups every day, restore with a preview, and optional encrypted cloud backup.

## Download

Get the latest version from the [Releases page](https://github.com/schwimmbeck/vaultime/releases/latest). Vaultime updates itself when a new version is out.

### Windows

Windows 10 and 11 are supported. Download `Vaultime_x.y.z_x64-setup.exe` and run it. No admin rights are needed. The `.msi` package is there for managed installs.

### Linux

| Distribution | Package |
|---|---|
| Debian, Ubuntu, Linux Mint, Pop!_OS | `.deb` |
| Fedora | `.rpm` |
| Arch, openSUSE and any other | `.AppImage` |

For the AppImage, make it executable (`chmod +x Vaultime_*.AppImage`) and start it.

On X11 Vaultime asks the X server which window is in front and when you last used the keyboard or mouse, nothing extra to install. Games that keep the screensaver off still turn idle when you leave. On Wayland the same works for games that run through XWayland, which is most of them, and GNOME also reports how long you have been away from any window.

Signing in to cloud backup needs a keyring service such as GNOME Keyring or KWallet. Most desktops ship one.

## How playtime is counted

| Time | Meaning |
|---|---|
| Runtime | How long the game was running |
| Active | Runtime while the game was in front and you were at your computer |
| Idle | Runtime while the game sat in the background or you were away |

You count as away after 5 minutes without keyboard, mouse or controller input. On Windows, controller input means Xbox controllers and controllers that act as one. On Linux it means every controller. You can change the 5 minutes in Settings, and you can also choose to count background time as active.

## Trust labels

Every session carries a label that tells how much its record can be trusted:

- **Local.** Recorded normally and unchanged since.
- **Suspicious.** The system clock jumped or the record was edited afterwards.
- **Recovered.** Vaultime was closed while the game ran, for example after a crash, and the session was closed on the next start.

These labels detect changes, they cannot prevent them. They are hints, not proof.

## Backups

- **Local backups** are folders you can keep anywhere. Vaultime makes one by itself once a day and when it quits, and keeps the newest seven. Pick their folder under Settings, Local backups, ideally on another drive or in a folder that syncs. Save a backup makes one by hand, and Restore from a backup shows what is inside before anything is replaced.
- **Cloud backup** is free and invite-only. Backups are encrypted on your computer with your backup passphrase before they are uploaded, so nobody else can read them. Keep the passphrase safe, without it a backup cannot be restored. When you reach the backup limit, a new upload replaces your oldest one. You can also delete backups yourself.

## Your data

| | Windows | Linux |
|---|---|---|
| Library and history | `%APPDATA%\com.vaultime.app` | `~/.local/share/com.vaultime.app` |
| Log files | `%LOCALAPPDATA%\com.vaultime.app\logs` | `~/.local/share/com.vaultime.app/logs` |

Vaultime has no analytics and no telemetry. Details are in the [privacy policy](docs/legal/privacy-policy.md) and the [terms](docs/legal/terms-of-service.md).

## Platform support

| Platform | Status |
|---|---|
| Windows 10 and 11 | Supported |
| Linux on X11 | Supported |
| Linux on Wayland | Works for games that run through XWayland, which covers most Proton games. Native Wayland windows and idle time can be misread. |
| macOS | Not available yet |

## License

[MIT](LICENSE)
