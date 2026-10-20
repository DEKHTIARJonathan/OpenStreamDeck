# Changelog

The most important OpenStreamDeck product updates are listed here. This is a
curated, high-level release history; routine maintenance and implementation
details are intentionally omitted.

## Unreleased

### Features

### UI/UX

### Performance

### Fixes

## v0.2.0 - 2026-10-20

### Features

- Signed automatic updates from GitHub releases, with release notes and an
  option to skip a version.
- Application-specific pages automatically follow the foreground application,
  with a Default fallback for applications without an assigned page.
- Each application can have multiple pages, with Previous/Next navigation
  cycling through that application's saved page order.

### UI/UX

- A clean, optional first-launch setup guides platform-specific input access and
  launch at login, with clear skip choices remembered across updates.
- Separate, aligned Applications and Pages rows make application contexts and
  their pages easier to manage.
- Drag and drop to reorder pages, with visible Add page, Rename page and Delete
  controls. Deleting an application removes all its pages after confirmation;
  Default is protected.

### Fixes

- Keep saved page settings intact when opening configurations from another
  build.
- Preserve newer configuration changes while an application lookup is pending.
- Keep ordinary buttons and dials on the current application's bindings while
  the Agents overlay is displayed.

## v0.1.0 - 2026-10-20

Initial Release - Support for MiraBox N3 devices.
