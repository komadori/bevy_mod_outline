# bevy_mod_outline

This crate provides a Bevy plugin for drawing outlines around meshes using the
vertex extrusion method.

## Dependency

```toml
[dependencies]
bevy_mod_outline = "0.2"
```

## Example

A rotating rounded cube with an outline.

```shell
cargo run --example cube
```

## Versions

| This Version | Bevy version |
|--------------|--------------|
| 0.1.x        | 0.7.x        |
| 0.2.x        | 0.8.x        |

## Known Issues

Vertex extrusion only works for meshes with smooth surface normals. Hard edges
will cause visual artefacts.
