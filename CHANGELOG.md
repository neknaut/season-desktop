# Changelog

All notable changes to this project are documented in this file.
The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project uses
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.0] - 2026-11-25

First public version.

### Added

- Live wallpaper rendered under the desktop icons, one scene per monitor, landscape and portrait layouts.
- 16 themes: four seasons and Vietnamese / international holidays, with painted backgrounds.
- Automatic mode: seasonal theme on normal days, holiday theme ahead of each holiday
  (3 days; 1 week for Christmas and New Year; 1 month for Lunar New Year).
- Only meaningful motion is animated (falling apricot blossoms at Lunar New Year, snow and Santa's sleigh
  at Christmas, fireworks at New Year, falling leaves in autumn, sky lanterns at Mid-Autumn);
  static scenes redraw only every 2 seconds.
- Optional per-monitor "three island" taskbar that replaces the Windows taskbar:
  - pinned and running apps read from Windows (same apps, same order as the Windows taskbar),
    window grouping, live previews, jump lists, drag to reorder;
  - system tray host that forwards every icon to Windows, hidden-icons flyout with a rotating chevron;
  - network, volume, battery, clock and calendar;
  - per-theme decorations.
- Settings window: per-monitor theme gallery, loading screen, close-to-tray or quit on close, effects and
  frame-rate limits, start with Windows.
- Per-user MSI installer (no admin rights) built with WiX 3.14; uninstall restores the Windows taskbar and
  wallpaper.

### Known limitations

- Pinning and unpinning must be done on the Windows taskbar or Start menu: Windows does not allow other apps to
  pin, and unpinning through the API makes Windows rewrite the pin list.
- Tray icons owned by `SYSTEM` processes (for example NVIDIA) or sent without a callback (Windows Security) cannot
  show their own right-click menu; clicking them opens the app instead.

[0.1.0]: https://github.com/neknaut
