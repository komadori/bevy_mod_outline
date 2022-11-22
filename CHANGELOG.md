# Changelog

## bevy_mod_outline 0.3.0 (2022-11-22)

### Added
- Added ComputedOutlineDepth, SetOutlineDepth, and InheritOutlineDepth.
- Added offset field to OutlineStencil.
- Added pieces example.

### Removed
- Removed align16 feature.

### Fixed
- Fixed errant debug println in AutoGenerateOutlineNormalsPlugin.

### Changed
- Updated Bevy dependency to 0.9.
- Renamed Outline component to OutlineVolume.

## bevy_mod_outline 0.2.4 (2022-10-12)

### Fixed
- Fixed adding outlines to jointed (skinned) meshes.

## bevy_mod_outline 0.2.3 (2022-08-28)

### Added
- Added AutoGenerateOutlineNormalsPlugin.

## bevy_mod_outline 0.2.2 (2022-08-23)

### Added
- Added a feature flag to control the dependency on bevy_ui (@Shatur).
- Added a feature flag to control uniform struct alignment.

### Fixed
- Fixed compilation on 32-bit platforms.
- Fixed insufficient alignment causing errors with WebGL.

### Changed
- Removed dependency on bevy's monolithic render feature flag (@Shatur).

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
