# dyn_shim

[![crates.io](https://img.shields.io/crates/v/dyn_shim.svg)](https://crates.io/crates/dyn_shim)
[![docs.rs](https://img.shields.io/docsrs/dyn_shim)](https://docs.rs/dyn_shim)
[![CI](https://github.com/nixpulvis/dyn_shim/actions/workflows/ci.yml/badge.svg)](https://github.com/nixpulvis/dyn_shim/actions/workflows/ci.yml)
[![license](https://img.shields.io/crates/l/dyn_shim.svg)](LICENSE)

Generate a dyn-compatible shim trait and blanket impl from a source trait that
is not dyn-compatible.

Some traits are not dyn-compatible, so you cannot hold a mixed set of
implementors behind one `Box<dyn Trait>`. The `#[dyn_shim(Name)]` attribute
reads the trait it is applied to, builds a second trait containing only the
dyn-compatible subset, and forwards each call to the original. Every implementor
of the source trait then works as a `dyn` shim.

## Usage

Add the dependency:

```toml
[dependencies]
dyn_shim = "0.1"
```

Annotate the trait with `#[dyn_shim(Name)]`, where `Name` is the shim trait to
generate:

```rust
use dyn_shim::dyn_shim;

#[dyn_shim(DynSink)]
trait Sink {
    // ...
}

See the [API documentation](https://docs.rs/dyn_shim) for details.

## Testing

```sh
cargo test
```

The suite includes [`trybuild`](https://crates.io/crates/trybuild) UI tests
under `tests/ui/` that assert the compile errors for rejected traits and methods.

## License

Licensed under the MIT license. See [LICENSE](LICENSE) for details.
