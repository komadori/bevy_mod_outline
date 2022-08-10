# Changelog

## bevy_mod_outline 0.2.1 (2022-08-10)

### Added
- Added a constant to expose the outline pass node name.

### Fixed
- Fixed outlines causing UI to disappear when MSAA is enabled.
- Fixed bad derive allowing OutlineBundle to be inserted as a component.

## bevy_mod_outline 0.2.0 (2022-08-09)

### Added
- Added support for outline normals.

### Changed
- Updated Bevy dependency to 0.8.
- Changed from rendering in main pass to separate pass.
- Changed from using asset handles to plain components.

## bevy_mod_outline 0.1.0 (2022-06-14)

- Initial release
