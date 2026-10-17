# Getting started

Install Vaultime, let it find your games and play the way you always do. It notices when a game starts and records the session on its own.

## Install

Vaultime is free. Get it from the [download section](https://vaultime.codfishcloud.de/#download) of this site or from the [releases on GitHub](https://github.com/vaultime/vaultime/releases/latest), which have the same installers.

### Windows

Windows 10 and 11 are supported. Download `Vaultime-setup.exe` and run it. It installs for your user account, so no admin rights are needed. The `.msi` package is there for managed installs.

### Linux

| Distribution | Package |
|---|---|
| Debian, Ubuntu, Linux Mint, Pop!_OS | `.deb` |
| Fedora | `.rpm` |
| Arch, openSUSE and any other | `.AppImage` |

The AppImage needs to be made executable once:

```
chmod +x Vaultime.AppImage
./Vaultime.AppImage
```

Every release is installed and started on Debian, Ubuntu, Fedora, Arch and openSUSE before it is published. What works where on Linux, X11 and Wayland is on [Platforms and Linux](platforms.md).

### Updates

Vaultime looks for a new version when it starts and every few hours. When one is out, a banner offers **Update and restart**. You can also look by hand under Settings, About, with **Check now**.

## First start

The library starts empty and says so: **Start your library**, with two buttons, **Discover games** and **Add a game**. There is no account and no setup wizard. Vaultime starts watching as soon as a game is in your library.

## Discover your games

1. Pick **Discover games**.
2. Pick **Start the scan**. Vaultime reads what your launchers installed and looks through the usual install folders.
3. Check the games you want and pick **Add games**.

Games found through a launcher start checked. Games found in a plain folder start unchecked, because such folders also hold tools, setup programs and mod loaders. Games you already have are marked **In library**.

Launchers read directly
: Steam, the Xbox app, Epic, GOG, EA, Ubisoft Connect, Battle.net, Riot, Rockstar, Amazon Games, itch.io, HoYoPlay, Legacy Games, Big Fish, Heroic and Lutris. Flatpak installs of Steam, Heroic, Lutris and itch.io are found too.

Folders scanned on Windows
: `Games`, `Epic Games`, `GOG Games`, `XboxGames` and the default game folders of Epic, GOG Galaxy, EA, Ubisoft and Battle.net, on every fixed drive.

Folders scanned on Linux
: `~/Games`, `~/games`, `/opt/games` and `/usr/games`.

You can scan again at any time with **Discover** above the game grid, or from the command palette.

### New games find you

Each time Vaultime starts, it reads what Steam and your other launchers installed, a few seconds after it opens and without holding anything up. When it finds games that are not in your library yet, it shows them in the same dialog, titled **New games found**, with all of them checked. Plain game folders are left out of this look, because folder finds are guesses and scanning every drive at each start would be slow.

Each game is offered only once on a PC. A game you leave out, or one you already saw in a full scan, stays quiet from then on, and **Discover** still lists everything. When Vaultime starts in the tray at login, the dialog waits until you open the window. The switch **Look for new games when Vaultime starts** under Settings, In the background turns it off.

### The same game from two launchers

When a scan finds a game from one launcher that is in your library from another, like Cyberpunk 2077 on GOG next to your Steam copy, the list says **Same game as Cyberpunk 2077 on Steam**. Adding it links it to that game, see [One game, two installs](game-pages.md#one-game-two-installs). Untick the line to add it as a game of its own. Vaultime only suggests this when the title matches exactly one game from another launcher.

## Add a game by hand

**Add a game** asks for three things:

Title
: How the game is called in your library. The only field you have to fill in.

Program
: The file that runs while you play. Vaultime counts time while this program runs. On Windows that is an `.exe`, on Linux any executable file or script.

Install folder
: The game's folder. Vaultime looks in it for cover art.

Without a program, Vaultime cannot notice the game running, but you can still add play to it by hand.

## Not a game? Ignore it

Discovery sometimes finds a benchmark, a launcher or a tool. Point at it in the discover list and pick **Ignore**, and Vaultime never suggests it again and never tracks it. Deleting a game offers the same with the box *It is not a game*.

Ignored programs are listed under Settings, Ignored programs, where **Allow again** takes one back.

## Hide a game

Hiding is for privacy, not for programs that are no games. A hidden game is left out of the library, the rail and search, but Vaultime still tracks it and counts its time. Hide it on its page under Tracking. Its page, or Settings, Hidden games, brings it back with **Show in library**.

## Get around

- ++Ctrl+K++ opens the command palette. It finds any game, page or setting and runs common actions like **Save a backup** or **Check for updates**.
- ++Ctrl+F++ finds a game in your library.
- The live bar at the bottom shows the running game with its timer and how much of it was active.

## In the background

Closing the window keeps Vaultime running in the tray, and it starts when you log in. Both are switches under Settings, In the background: **Keep tracking when the window is closed** and **Start with Windows**, called **Start when you log in** on Linux. The tray menu shows the game that runs and how long you played today.

On Linux the tray needs `libayatana-appindicator3`, and GNOME shows tray icons only with an AppIndicator extension. Ubuntu ships one.

## Make it yours

Settings, Appearance picks dark, light or the system's mode, one of six ground tones, an accent color and a background picture with its own dim and blur. Settings, Window picks a fixed window size, from Compact at 1024 by 640 to Extra large at 1920 by 1200, or Free for a window you resize yourself.

The look, the background picture and the window size belong to the PC you set them on. Backups leave them out.
