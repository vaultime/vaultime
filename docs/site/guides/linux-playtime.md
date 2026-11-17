# How to see your playtime on Linux

On Linux your games are often spread over Steam, Heroic and Lutris, and each launcher keeps its own count. Here is where each one shows it and what it counts.

## Heroic Games Launcher

Open a game in Heroic. Its page shows **Time played**, **Last played** and **First played**.

Heroic counts every game you start from it, from Epic, GOG and Amazon and games you added yourself, from start until the game closes, and keeps the time on your PC.

- For GOG games it also sends your sessions to GOG and shows GOG's total when that is larger, so time from GOG Galaxy on Windows shows up too. **Disable playtime synchronization** in its settings turns that off.
- Time of Epic and Amazon games stays in Heroic. It is not read from those stores.

## Lutris

1. Switch Lutris to the list view. The **Play Time** and **Last Played** columns show each game's total.
2. Pick a game, and the bar below shows the time played and when you last played it.

Lutris counts the time a game you started from it runs and keeps it on your PC, with no history of your days.

## Steam

Steam on Linux counts as it does on Windows, Proton games included: one total for each game on its page in your library. See [How to see your playtime on Steam](steam-playtime.md).

## What Vaultime adds

[Vaultime](/) runs on Linux as a `.deb`, an `.rpm` and an AppImage. It finds the games of Steam, Heroic and Lutris, Flatpak installs included, and counts the game that runs, whichever launcher started it.

- Active time and idle time apart, with controller input counting as activity.
- It works on X11, and on Wayland for games that run through XWayland, which most games do, Proton games included. [Platforms and Linux](platforms.md) has the details for each desktop.
- One library and one journal of your days for every launcher.

See also [How to see all your playtime in one place](all-your-playtime.md).

This page was checked in November 2026. Launchers change their apps, so some labels may differ.
