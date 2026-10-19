# Troubleshooting

Answers to the questions that come up most, and where to look when tracking does not do what you expect.

## Where to look first

Settings, Detection
: Shows whether tracking runs and how Vaultime tells which window is in front, whether you are there and whether it sees controllers.

The game's page
: Under Tracking it shows the program Vaultime watches and how the game is recognized. The event log lists the newest events recorded for its sessions, such as flags and corrections.

The log files
: Vaultime writes what it does to log files. On Windows they are in `%LOCALAPPDATA%\com.vaultime.app\logs`, on Linux in `~/.local/share/com.vaultime.app/logs`. A file is rotated at about 2 MB and the last three are kept.

## Tracking

### A game does not show up in Discover

Add it by hand with **Add a game** and pick its program. Discovery reads the common launchers and a few usual folders, so games installed elsewhere are not found. See [Getting started](getting-started.md#add-a-game-by-hand).

### A game runs but no session starts

- Open the game's page and check the program under Tracking. After an update or a move to another drive the game may start from a new path. **Edit** points it at the right one.
- Some games start through a small launcher program first. If the game came from a launcher Vaultime reads, any game program in its install folder counts. For a game you added by hand, pick the program that runs while you play, not the launcher.
- Check that the program is not listed under Settings, Ignored programs.
- Games that came from another PC are never tracked here.
- Settings, Detection should say that tracking runs.

### Time counts as idle while I play

- With a controller on Windows, only XInput controllers are seen: Xbox controllers and controllers that act as one. PlayStation and Switch controllers that only Steam Input turns into an Xbox controller are not seen, because Steam does that inside the game.
- On Linux, check that your user can read the controller's device under `/dev/input`. The log says when it cannot.
- In a game you play in a window next to other things, switch on **Count background games as active**.
- If you watch cutscenes or wait in queues without touching anything, raise **Count as idle after** under Settings, Tracking.

### Controller play counts as idle

See the first two points above. Vaultime also ignores small stick and trigger movement, so a controller resting on the table does not keep a game active.

### Idle time looks wrong on Wayland

Native Wayland windows are hard to see for any app. Outside GNOME, Vaultime then falls back to the game's own activity, see [Platforms and Linux](platforms.md#linux-on-wayland). Games that run through XWayland are tracked in full, and many games can be started under XWayland, for example SDL games with `SDL_VIDEODRIVER=x11`.

### A launcher counts as playtime

Open the launcher's game page and switch on **Count only when no other game runs**. Its time beside other games is set aside. If the launcher is not a game at all, delete it and check *It is not a game*.

### A game left running overnight counted as play

Most of that time should be idle already. To take it out, point at the session and pick **Correct the time**, then **Count it only until a time**. See [Correct a session](journal-and-stats.md#correct-a-session).

## Labels

### Why is a session Suspicious?

The reason is under the session in the journal and in the game's event log. The usual cause is a system clock that jumped while the game ran, for example when Windows or a time service corrected a clock that was far off. The label stays, even after a correction, so that a flag cannot be hidden. See [Trust labels](trust-labels.md#what-makes-a-session-suspicious).

### Why is a session Recovered?

Vaultime was not running when the session ended, for example after a crash, a power cut or when it was ended from the task manager. It closed the session on its next start with the time up to its last note.

### Settings says sessions are missing

Sessions the ledger knows are gone from the history without a note that you removed them. That happens when the database was edited, replaced with an older copy outside Vaultime or damaged. A backup from before it happened still holds them.

## Cloud backup

### Signing in fails on Linux

The sign-in is kept in the system keyring. If an error mentions secure storage, install and start a keyring service such as GNOME Keyring or KWallet.

### I forgot my backup passphrase

It cannot be recovered, and cloud backups made with it cannot be restored. Your local history and local backups do not depend on it.

## Starting

### Vaultime says it could not open its database

The database file is damaged, or another program holds it. When another program may be using it, such as a sync tool or a virus scanner, choose **Quit** and start Vaultime again later. Otherwise choose **Restore the backup**: Vaultime renames the database to `vaultime.db.damaged-` followed by the time, keeps it next to the new one and starts from the newest automatic backup. Play recorded after that backup stays only in the renamed file. Nothing is deleted, and when the restore fails, everything stays as it was.

### Vaultime says it is older than the one that last used its data

A newer version of Vaultime changed the database, and an older one would not keep what it added. Install the newest version from the [website](https://vaultime.codfishcloud.de/#download).

## Window

### The window is too large or too small

Settings, Window picks a fixed size from Compact at 1024 by 640 to Extra large at 1920 by 1200. A size that does not fit the screen steps down to the next one. **Free** lets you resize and maximize the window yourself.

### No tray icon on Linux

Install `libayatana-appindicator3`. Without it there is no tray, and closing the window quits Vaultime, so tracking stops until you start it again. On GNOME you also need an AppIndicator extension. Without one, Vaultime keeps running after you close the window, only its icon does not show. Start Vaultime again to bring the window back.

## Still stuck?

Open an issue on [GitHub](https://github.com/vaultime/vaultime/issues) with what you expected, what happened, your system and the log files. Please report security issues privately as described in the [security policy](https://github.com/vaultime/vaultime/security/policy).
