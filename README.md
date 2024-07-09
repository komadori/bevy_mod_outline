# bevy_mod_outline

[![crates.io](https://img.shields.io/crates/v/bevy_mod_outline.svg)](https://crates.io/crates/bevy_mod_outline)
[![docs](https://docs.rs/bevy_mod_outline/badge.svg)](https://docs.rs/bevy_mod_outline)

![Screenshot of bevy_mod_outline's shapes example](https://github.com/bevyengine/bevy-assets/blob/main/Assets/3D/bevy_mod_outline.png?raw=true)

This crate provides a Bevy plugin for drawing outlines around meshes using the
vertex extrusion method.

## Dependency

```toml
[dependencies]
bevy_mod_outline = "0.8"
```

## Examples

A rotating cube and torus with opaque and transparent outlines.

```shell
cargo run --example shapes
```

Multiple intersecting meshes sharing an outline plane. The outline stencil is offset to create
a gap between the object and its outline.

```shell
cargo run --example pieces
```

Many instances of the same mesh, with two different outline configurations, flying towards the
camera.

```shell
cargo run --example flying_objects
```

An outlined torus viewed through four cameras with different combinations of render layers
enabled.

```shell
cargo run --example render_layers
```

An animated jointed glTF model with an outline.

```shell
cargo run --example animated_fox
```

A glTF model with pre-baked outline normals.

```shell
cargo run --example hollow
```

An animated morphing glTF model with an outline.

```shell
cargo run --example morph_targets
```

A pair of spheres, one outlined, with a UI for selecting different anti-aliasing modes.

```shell
cargo run --example ui_aa
```

An emissive sphere orbits another sphere, with outlines and HDR bloom post-processing.

```shell
cargo run --example bloom
```

## Versions

| This Version | Bevy version |
|--------------|--------------|
| 0.8.x        | 0.14.x       |
| 0.7.x        | 0.13.x       |
| 0.6.x        | 0.12.x       |
| 0.5.x        | 0.11.x       |
| 0.4.x        | 0.10.x       |
| 0.3.x        | 0.9.x        |
| 0.2.x        | 0.8.x        |
| 0.1.x        | 0.7.x        |

## Features

- `interpolation` _(default)_ - Define `Lerp` trait impls using the
`interpolation` crate.

## Licence

This crate is licensed under the Apache License, Version 2.0 (see
LICENCE-APACHE or <http://www.apache.org/licenses/LICENSE-2.0>) or the MIT
licence (see LICENCE-MIT or <http://opensource.org/licenses/MIT>), at your
option.

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall
be dual licensed as above, without any additional terms or conditions.
