//! Converting the `dyn_clone` README example to `#[dyn_shim]`, and why the
//! result is better.
//!
//! The original program:
//!
//! ```ignore
//! use dyn_clone::DynClone;
//!
//! trait MyTrait: DynClone {
//!     fn recite(&self);
//! }
//!
//! impl MyTrait for String {
//!     fn recite(&self) {
//!         println!("{} reporting in", self);
//!     }
//! }
//!
//! fn main() {
//!     let line = "The slithy structs did gyre and gimble the namespace";
//!     let x: Box<dyn MyTrait> = Box::new(String::from(line));
//!     x.recite();
//!     let x2 = dyn_clone::clone_box(&*x);
//!     x2.recite();
//! }
//! ```
//!
//! `#[dyn_shim(DynMyTrait: Clone)]` replaces both the `: DynClone` supertrait
//! and the `clone_box`/`clone_trait_object!` machinery, and improves on it:
//!
//! - No `unsafe`. `dyn_clone` reconstitutes a cloned `Box<dyn MyTrait>` with a
//!   raw fat-pointer splice. The generated shim clones through a safe coercion,
//!   because the clone method lives on `DynMyTrait` itself, so the compiler
//!   builds the `Box<dyn DynMyTrait>` with no pointer surgery.
//! - Nothing extra to call. The attribute emits the `Clone` impl directly, plus
//!   a `ToOwned` impl for `dyn DynMyTrait` (which `dyn_clone` does not provide),
//!   so a borrowed `&dyn DynMyTrait` can escape as an owned box.
//! - The `Clone` bound applies only to types used through the shim, not to every
//!   `MyTrait` implementor. A non-`Clone` type may still implement `MyTrait`; it
//!   simply never becomes a `DynMyTrait`. To keep the original "all implementors
//!   are `Clone`" contract, also write `trait MyTrait: Clone`.
//!
//! And nothing is lost: with the `dyn_clone` feature, `DynMyTrait` is a
//! `dyn_shim::DynClone`, so the shim still satisfies `DynClone` bounds. A
//! `Box<dyn DynMyTrait>` works wherever a `DynClone` bound or `Box<dyn
//! DynClone>` is expected (see the tail of `main`).
//!
//! The conversion: swap the import for `dyn_shim`, move `: DynClone` into the
//! attribute as `: Clone`, drop the `clone_box` call in favor of `.clone()`
//! (`.to_owned()` on a borrow), and write `dyn DynMyTrait` for the trait object.
//!
//! Run with: `cargo run --example migrate_dyn_clone` (add `--features dyn_clone`
//! for the `DynClone`-bound interop at the end).

use dyn_shim::dyn_shim;

// Was: trait MyTrait: DynClone {
#[dyn_shim(DynMyTrait: Clone)]
trait MyTrait {
    fn recite(&self);
}

// Unchanged.
impl MyTrait for String {
    fn recite(&self) {
        println!("{} reporting in", self);
    }
}

fn main() {
    let line = "The slithy structs did gyre and gimble the namespace";

    // Was: let x: Box<dyn MyTrait> = ...;
    let x: Box<dyn DynMyTrait> = Box::new(String::from(line));
    x.recite();

    // Was: let x2 = dyn_clone::clone_box(&*x);
    let x2 = x.clone();
    x2.recite();

    // Our shim still satisfies a `DynClone` bound, with no `unsafe` and no
    // `clone_trait_object!`: `Box<dyn DynMyTrait>` is a `dyn_shim::DynClone`.
    #[cfg(feature = "dyn_clone")]
    {
        fn needs_dyn_clone<T: dyn_shim::DynClone>(_: T) {}
        let x3: Box<dyn DynMyTrait> = Box::new(String::from(line));
        needs_dyn_clone(x3);
    }
}
