# Changelog

All notable changes to Vaultime will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

The first public release.

### Tracking

- **Automatic tracking.** Start a game however you usually do and Vaultime records the session. Runtime, active playtime and idle time are counted apart.
- **Keeps running in the tray.** Closing the window keeps tracking, and Vaultime starts with Windows or when you log in on Linux. Both can be turned off in Settings.
- **Light on your PC.** A check for running games takes a few milliseconds every five seconds, and the window stops drawing while it sits in the tray.
- **Sleep aware.** Time while your computer sleeps is not counted.
- **Controller play counts.** Buttons and sticks count as input, so a game played on a controller does not turn idle. On Windows this covers Xbox controllers and controllers that act as one.
- **Nothing extra to install on Linux.** Vaultime asks the X server directly which window is in front and when you last used the keyboard or mouse, and GNOME on Wayland. Games that keep the screensaver off still turn idle when you leave.
- **Careful matching.** Games are matched by their full path, so two games with the same file name stay apart. Libraries behind junctions or symlinks and games that run through Wine still match.
- **Honest trust labels.** Every session keeps a hash-linked event log. Sessions with a clock jump are marked Suspicious, sessions rebuilt after an unclean exit Recovered, and both say what happened.

### Library

- **Library home.** Opens on the game you play right now or last, in colors taken from its cover, with the last seven days in numbers, a shelf of recent games and every game below.
- **Game pages.** The last two weeks at a glance, every session as a sentence, totals and the cover you pick.
- **Journal.** Your play week by week, with a 24 hour strip for each day.
- **Live bar and search.** The running game and its timer stay at the bottom of every page, and Ctrl+K jumps to any game, page or action.
- **Game discovery.** Reads what Steam, the Xbox app, Epic, GOG, EA, Ubisoft Connect, Battle.net, Riot, Rockstar, Amazon Games, itch.io, HoYoPlay, Legacy Games, Big Fish, Heroic and Lutris have installed, and scans common game folders on every drive for the rest.
- **Steam covers.** Steam games get the cover art Steam already keeps on your PC.
- **Artwork.** Picks up cover art from your game folders, or add your own.
- **Hidden games.** Hide a game from the library without losing it. It stays tracked, its sessions still count in the journal and your totals, and Settings lists hidden games to bring them back.

### Backups

- **Local backups** to any folder, with a checksum for every file and a preview before a restore replaces anything.
- **Automatic backups** once a day and when Vaultime quits, the newest seven kept, in Vaultime's data folder or a folder you pick. Artwork that did not change is stored once for all of them. Signed in PCs also back up to the cloud once a day.
- **Invite-only cloud backup** on our own server. Free, with the same limits for every account. Backups are encrypted on your computer before upload, artwork is uploaded once and shared by every backup, and at the limit a new backup replaces the oldest.
- **Password change** on the Cloud page. Your other PCs are signed out, and the backup passphrase stays the same.

### Platforms

- **Windows** with an installer for your user account.
- **Linux** as .deb, .rpm and AppImage. Every release is installed and started on Debian, Ubuntu, Fedora, Arch and openSUSE before it is published.
