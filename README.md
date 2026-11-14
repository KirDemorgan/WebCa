<div align="center">

# WebCa

**Set your webcam once. Keep it everywhere.**

A small, native Windows 11 app for webcam image settings — brightness, exposure, white balance,
focus and the rest — that stay put in Discord, OBS, Telegram, Teams and every other app.

[Русская версия](README.ru.md) · [Contributing](CONTRIBUTING.md) · [Changelog](CHANGELOG.md)

</div>

---

## Why

Windows still hides camera settings behind a property sheet from the early 2000s, and whatever you
set there lasts only until the next app opens the camera, unplugs it, or the PC wakes from sleep.
WebCa replaces that dialog with a modern window and makes your settings stick.

## Features

- **Every control your camera has** — the full *Video Proc Amp* and *Camera Control* sets:
  brightness, contrast, hue, saturation, sharpness, gamma, white balance, backlight compensation,
  gain, anti-flicker, exposure, focus, zoom, pan and tilt, with automatic modes where supported.
- **Live preview** while you adjust.
- **Applied in every app.** Save once; Windows itself applies your values whenever any app starts
  the camera (system-wide mode), or WebCa's tray agent does it for you.
- **Lock values.** Optionally undo changes other apps make to your camera while they use it.
- **Profiles** — for example *Day* and *Evening* — switchable from the window or the tray.
- **Command line and MCP server** for scripts, stream decks and AI assistants.
- **Native and light.** Written in Rust with a GPU-rendered interface; follows the system light or
  dark theme. English and Russian.
- **Private.** No accounts, no telemetry. The only network request is an optional check for a
  newer release on GitHub.

## How it works

```
                 you press Save
                       |
          +------------+-------------+
          |                          |
   system-wide mode              tray agent
          |                          |
  Windows camera defaults     watches camera activity
  store (per user, per        and re-applies the profile
  camera)                     when another app starts
          |                   streaming; with Lock on,
          |                   undoes foreign changes
          +------------+-------------+
                       |
     Discord / OBS / Telegram / Teams start the camera
                       |
               your settings are there
```

**System-wide mode** uses the camera defaults framework that Windows 11 introduced for its own
camera settings page. WebCa registers itself as your camera's companion app, which needs the
installed (MSIX) version and a one-time administrator confirmation. After that no WebCa process
has to run for your settings to apply.

**The tray agent** works with the portable version too. It uses almost no resources while the
camera is idle and steps in the moment any app starts streaming.

Apps that set their own camera values (OBS can, for example) take precedence over Windows
defaults; turn on **Lock values** if you want WebCa to keep yours.

## Install

Download the latest release from the **Releases** page:

| Package | Use it when |
|---|---|
| `WebCa-x.y.z-x64.msix` | You want system-wide mode and automatic start. Double-click to install. |
| `WebCa-x.y.z-portable-x64.zip` | You prefer a single executable. Settings are kept by the tray agent. |

Releases are currently published as pre-releases. The MSIX package is not yet signed by a trusted
certificate, so Windows installs it only with Developer Mode turned on; the portable archive works
everywhere.

Requirements: Windows 11 (x64) and a UVC webcam. Windows 10 is not supported yet — see
[docs/windows-10-gaps.md](docs/windows-10-gaps.md).

## Use

1. Open WebCa, pick your values while watching the preview.
2. Press **Save**.
3. Optionally press **Turn on system-wide mode** (installed version) and confirm the Windows prompt.

Closing the window keeps WebCa in the notification area. Its menu switches profiles, toggles
**Lock values**, reopens the window and quits.

### Command line

```
webca list                       every control with range and current value
webca get exposure               current value of one control
webca set exposure -6            change a control right now
webca set white_balance auto     let the camera decide
webca profiles                   saved profiles, the active one marked with *
webca apply Evening              apply a saved profile
```

### MCP server

`webca mcp` speaks the Model Context Protocol over standard input and output, so an assistant can
read and adjust your camera. Add it to your MCP client configuration:

```json
{
  "mcpServers": {
    "webca": { "command": "C:\\Path\\To\\webca.exe", "args": ["mcp"] }
  }
}
```

Tools: `list_controls`, `get_control`, `set_control`, `reset_control`, `list_profiles`,
`apply_profile`.

## Where settings live

Profiles are stored in `%APPDATA%\WebCa\profiles.toml`. System-wide defaults live in Windows and
can be cleared from **Settings › Bluetooth & devices › Cameras › Reset settings**.

## Build from source

```
cargo run -p webca
```

See [CONTRIBUTING.md](CONTRIBUTING.md) for tests, coverage and packaging.

## License

WebCa is free software, released under the [GNU General Public License v3.0 or later](LICENSE).
You may use, study, share and modify it; versions you distribute must remain open under the same
license.
