# dyn_shim

[![crates.io](https://img.shields.io/crates/v/dyn_shim.svg)](https://crates.io/crates/dyn_shim)
[![docs.rs](https://img.shields.io/docsrs/dyn_shim)](https://docs.rs/dyn_shim)
[![CI](https://github.com/nixpulvis/dyn_shim/actions/workflows/rust.yml/badge.svg)](https://github.com/nixpulvis/dyn_shim/actions/workflows/rust.yml)
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
dyn_shim = "0.2"
```

Annotate the trait with `#[dyn_shim(Name)]`, where `Name` is the shim trait to
generate:

```rust
use dyn_shim::dyn_shim;

#[dyn_shim(DynSink)]
trait Sink {
    // ...
}
```

Bounds after the shim's name become its supertraits. A `Clone` or `Hash` in
the list is recognized and handled specially: it makes the shim's trait
objects themselves cloneable (including `ToOwned`) or hashable, covering the
marker combinations of any auto traits listed alongside:

```rust
use dyn_shim::dyn_shim;

#[dyn_shim(DynShape: Clone + Send)]
trait Shape {
    fn area(&self) -> f64;
    fn scale(&mut self, factor: f64);
}

// Box<dyn DynShape> and Box<dyn DynShape + Send> implement Clone.
```

## Reflexive impls

By default the shim is a separate trait, so a `Box<dyn DynFoo>` is not a `Foo`.
Adding `reflexive = boxed` also generates `impl Foo for Box<dyn DynFoo>`, so the
boxed trait object satisfies the source trait itself and can be passed to code
written against `Foo`. Methods that cannot be dispatched through the shim (a
constructor, a generic method) are opted into a panicking stub with
`#[dyn_shim(panic)]`:

```rust
use dyn_shim::dyn_shim;

#[dyn_shim(DynMunch, reflexive = boxed)]
trait Munch {
    fn crunch(self) -> u32;
    #[dyn_shim(panic)]
    fn fresh() -> Self; // not dispatchable: panics if called on the box
}

fn eat(m: impl Munch) -> u32 {
    m.crunch()
}

// Box<dyn DynMunch> is a Munch, so it can be passed to `eat`.
```

`reflexive = bare` instead generates `impl Foo for dyn DynFoo`, so a `&dyn
DynFoo` satisfies `Foo` by reference. It cannot express a by-value `self` or a
`-> Self`, since `dyn DynFoo` is unsized; use `reflexive = boxed` for those.

## Foreign traits

`#[dyn_shim]` has to sit on the trait's own definition, so it cannot target a
trait from a dependency. `#[dyn_shim_foreign(path)]` does: the annotated trait
*is* the shim, restating the foreign methods to forward, and the macro fills in
the forwarding machinery plus a blanket impl pointing at the foreign path. Its
name, visibility, and supertrait list work just like `#[dyn_shim]`'s. A proc
macro cannot see another crate's trait body, so the signatures must be restated
by hand; a mismatch is caught when the generated forwarding call fails to
compile.

```rust
use dyn_shim::dyn_shim_foreign;

#[dyn_shim_foreign(other_crate::Sink)]
trait DynSink: Clone {
    fn write(&mut self, line: &str);
    fn finish(self) -> usize;
}

// Box<dyn DynSink> holds any Clone implementor of other_crate::Sink.
```

The `reflexive` option and `#[dyn_shim(panic)]` work on the foreign form too.

## Features

`Clone` and `Hash` cannot be supertraits of a dyn-compatible trait, so this
crate ships their shims directly, each behind a feature:

```toml
[dependencies]
dyn_shim = { version = "0.2", features = ["dyn_clone", "dyn_hash"] }
```

- `dyn_clone` provides `DynClone`: `Box<dyn DynClone>` implements `Clone` and `dyn
  DynClone` implements `ToOwned`. It is a drop-in for the `dyn-clone` crate's
  `DynClone`.
- `dyn_hash` provides `DynHash`: `dyn DynHash` implements `Hash` (covering `Box<dyn
  DynHash>` through the standard library's forwarding impl). It mirrors the
  `dyn-hash` crate.

With a feature on, a recognized `Clone`/`Hash` bound also makes the shim a
subtrait of `DynClone`/`DynHash`, so `Box<dyn DynFoo>` (or `&dyn DynFoo`)
upcasts to `Box<dyn DynClone>` (or `&dyn DynHash`) and flows into APIs typed
against those.

See the [API documentation](https://docs.rs/dyn_shim) for details.

## Testing

```sh
cargo test
```

The suite includes [`trybuild`](https://crates.io/crates/trybuild) UI tests
under `tests/ui/` that assert the compile errors for rejected traits and methods.

## License

Licensed under the MIT license. See [LICENSE](LICENSE) for details.
