# bevy_mod_outline

[![crates.io](https://img.shields.io/crates/v/bevy_mod_outline.svg)](https://crates.io/crates/bevy_mod_outline)
[![docs](https://docs.rs/bevy_mod_outline/badge.svg)](https://docs.rs/bevy_mod_outline)

This crate provides a Bevy plugin for drawing outlines around meshes using the
vertex extrusion method.

## Dependency

```toml
[dependencies]
bevy_mod_outline = "0.3"
```

## Example

A rotating cube and torus with opaque and transparent outlines.

```shell
cargo run --example shapes
```

Multiple intersecting meshes sharing an outline plane. The outline stencil is offset to create
a gap between the object and its outline.

```shell
cargo run --example pieces
```

An outlined torus viewed through four cameras with different combinations of render layers
enabled.

```shell
cargo run --example render_layers
```

An animated jointed model with an outline.

```shell
cargo run --features="bevy/animation bevy/bevy_gltf bevy/png bevy/bevy_scene" \
    --example animated_fox
```

## Versions

| This Version | Bevy version |
|--------------|--------------|
| 0.1.x        | 0.7.x        |
| 0.2.x        | 0.8.x        |
| 0.3.x        | 0.9.x        |

## Features

- `bevy_ui` _(default)_ - Adds a render graph edge to prevent clashing with the
UI. This adds a dependency on the `bevy_ui` crate and can be disabled if it is
not used.

## Licence

This crate is licensed under the Apache License, Version 2.0 (see
LICENCE-APACHE or <http://www.apache.org/licenses/LICENSE-2.0>) or the MIT
licence (see LICENCE-MIT or <http://opensource.org/licenses/MIT>), at your
option.

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall
be dual licensed as above, without any additional terms or conditions.
