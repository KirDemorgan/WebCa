# Windows 10 compatibility gaps

WebCa targets **Windows 11**. This page tracks every feature that depends on Windows 11 so that
Windows 10 support can be added later with clear fallbacks.

| Feature | Windows 11 mechanism | Windows 10 status | Possible fallback | Verified |
|---|---|---|---|---|
| System-wide camera defaults ("set once, applied by Windows") | Camera default values framework, `IMFCameraConfigurationManager` | Not available: the framework and the Settings › Cameras page were introduced in Windows 11 | Background agent re-applies the profile when the camera starts | Yes, Microsoft docs |
| Mica window backdrop | `WindowBackgroundAppearance::MicaBackdrop` | Not available | Solid or acrylic-like themed background | Yes, GPUI docs |
| Camera activity detection | `IMFSensorActivityMonitor` | Unknown | Poll `CapabilityAccessManager` consent store timestamps | To verify |
| External control change detection ("Lock" mode) | `IMFCameraControlMonitor` | Unknown | Periodic read-back of current values while the camera is active | To verify |
| Virtual camera for software effects (future stage) | `MFCreateVirtualCamera` | Believed unavailable | DirectShow virtual source filter | To verify |

When a row is verified on real Windows 10 hardware, update the **Verified** column and link the
evidence in the pull request.
