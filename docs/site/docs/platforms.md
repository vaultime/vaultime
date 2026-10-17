# Platforms and Linux

Windows and Linux are both supported in full. Every feature works on both, and every release is tested on both before it goes out.

## Support at a glance

| Platform | Status |
|---|---|
| Windows 10 and 11 | Supported |
| Linux on X11 | Supported |
| Linux on Wayland | Works for games that run through XWayland, which most games do, Proton games included. Native Wayland windows and idle time can be misread. |
| macOS | Not available yet |

Settings, Detection shows what Vaultime uses on your PC right now: how it tells which window is in front, how it tells whether you are there, whether it sees controllers and how often it checks.

## Windows

Vaultime asks Windows which window is in front and when you last used the keyboard or mouse. Controllers count through XInput, which covers Xbox controllers and controllers that act as one. It installs for your user account and updates without an admin prompt.

## Linux on X11

Vaultime asks the X server which window is in front and listens for raw input events to know when you last used the keyboard or mouse. There is nothing extra to install. Games that keep the screensaver off, as many do, still turn idle when you leave.

## Linux on Wayland

Wayland has no common way for an app to ask which window is in front. Vaultime still sees games that run through XWayland, which most games do, Proton games included, and their input counts as activity on every desktop.

- **GNOME** also reports how long you have been away from any window, so idle time is right even when you use native Wayland apps in between.
- **KDE, Sway, Hyprland and others** count input to XWayland windows. Input to native Wayland windows is not seen.

When Vaultime cannot tell which window is in front or whether you are there, it falls back to the game's own activity: a game counts as active while it uses the processor or while input came in recently. That keeps the totals close, but a game that keeps working in a menu while you are away can count as active.

## Controllers on Linux

Vaultime reads every device the system reports as a gamepad or joystick under `/dev/input`. It never opens keyboards or mice. Most desktops give the person at the PC access to game controllers. If yours does not, controller input is not seen and the log file says so.

## Flatpak and Proton

Steam, Heroic, Lutris and itch.io installed through Flatpak are found by discovery, and Vaultime finds the real program of games that run inside a Flatpak sandbox. Games that run through Proton or Wine are recognized by the game's own program, not by Wine's.

## Tray and keyring

- The tray icon needs `libayatana-appindicator3`. GNOME shows tray icons only with an AppIndicator extension, which Ubuntu ships.
- Signing in to cloud backup keeps your sign-in and the backup key in the system keyring, so it needs a keyring service such as GNOME Keyring or KWallet. Most desktops ship one.

## macOS

There is no macOS version yet. Parts of the code are ready for it, but nothing is built or tested for it.
