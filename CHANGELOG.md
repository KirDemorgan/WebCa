# Changelog

All notable changes to WebCa are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and the project uses
[Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added

- Settings window with live preview and every UVC image and camera control, grouped as Image,
  Light and Optics, with automatic modes where the camera supports them.
- Profiles with save, revert and factory reset.
- System-wide mode: the saved profile is written to the Windows camera defaults store, so Windows
  applies it whenever any app starts the camera (installed version, one-time administrator
  confirmation).
- Tray agent that re-applies the profile when another app starts streaming and, with *Lock values*,
  undoes changes other apps make; starts with Windows.
- Single running instance; launching again brings the window back.
- Command line: `list`, `get`, `set`, `profiles`, `apply`.
- Model Context Protocol server: `webca mcp`.
- Light and dark themes following Windows; English and Russian interface.
- Update check against GitHub releases.
- MSIX packaging script and release workflow.
