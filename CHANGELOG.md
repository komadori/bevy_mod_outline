# Changelog

## bevy_mod_outline 0.5.1 (2023-11-03)

### Added
- Added flying_objects example.

### Fixed
- Fixed Z-fighting between overlay and stencil more reliably.

### Changed
- Removed unnecessary extraction step.
- Removed unused vertex attributes bindings.

## bevy_mod_outline 0.5.0 (2023-08-14)

### Changed
- Updated Bevy dependency to 0.11. (@ramirezmike and @zainthemaynnn)
- Removed panic if specialising mesh pipeline fails. (@arjo129)

## bevy_mod_outline 0.4.3 (2023-11-06)

## Fixed
- Fixed Z-fighting between overlay and stencil more reliably (back-port from 0.5.1).

### Changed
- Removed unnecessary extraction step (back-port from 0.5.1).

## bevy_mod_outline 0.4.2 (2023-05-30)

### Fixed
- Fixed failures to propagate ComputedOutlineDepth when needed.
- Fixed Z-fighting between overlay and stencil with OpenGL wgpu back-end.

## bevy_mod_outline 0.4.1 (2023-04-11)

### Fixed
- Fixed panic if mesh has unused vertex indices.
- Fixed panic if the DepthMode hasn't propagated before rendering.

### Changed
- Changed normal weighting to use vertex rather than face normals if available.

## bevy_mod_outline 0.4.0 (2023-03-22)

### Added
- Added enabled flag to OutlineStencil.
- Added hollow example.

### Fixed
- Fixed outline depth propagating when inheritance not enabled.

### Changed
- Updated Bevy dependency to 0.10.
- Changed outline normal generator to use face normals.

## bevy_mod_outline 0.3.5 (2023-03-08)

### Fixed
- Fixed regression breaking SetOutlineDepth::Real.

## bevy_mod_outline 0.3.4 (2023-03-08)

### Fixed
- Fixed texture format error when HDR is enabled.
- Fixed bad clipping of outlines behind the camera.

## bevy_mod_outline 0.3.3 (2023-02-21)

### Fixed
- Fixed SetOutlineDepth being ignored in some cases (@mxgrey).
- Fixed defaulting to SetOutlineDepth::Real in some cases.
- Fixed missing examples from README.

## bevy_mod_outline 0.3.2 (2023-01-15)

### Added
- Added support for (Outline)RenderLayers components (@mxgrey).
- Added render_layers example.

## bevy_mod_outline 0.3.1 (2023-01-05)

### Added
- Added Lerp impls for OutlineVolume and OutlineStencil.
- Added Real mode to SetOutlineDepth enum.

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

## bevy_mod_outline 0.2.5 (2023-01-14)

### Added
- Added support for (Outline)RenderLayers components (@mxgrey) (back-port from 0.3.2).
- Added render_layers example (back-port from 0.3.2).

### Fixed
- Fixed errant debug println in AutoGenerateOutlineNormalsPlugin (back-port from 0.3.0).

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
