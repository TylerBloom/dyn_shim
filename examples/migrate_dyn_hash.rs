//! Converting the `dyn-hash` README example to `#[dyn_shim]`, and why the
//! result is better.
//!
//! The original program:
//!
//! ```ignore
//! use dyn_hash::DynHash;
//!
//! trait MyTrait: DynHash {
//!     /* ... */
//! }
//!
//! // Implement std::hash::Hash for dyn MyTrait
//! dyn_hash::hash_trait_object!(MyTrait);
//!
//! // Now data structures containing Box<dyn MyTrait> can derive Hash:
//! #[derive(Hash)]
//! struct Container {
//!     trait_object: Box<dyn MyTrait>,
//! }
//! ```
//!
//! `#[dyn_shim(DynMyTrait: Hash)]` replaces both the `: DynHash` supertrait and
//! the `hash_trait_object!` call, and improves on them:
//!
//! - Nothing extra to call. The attribute emits the `Hash` impl for the shim's
//!   `dyn` types directly, so there is no separate `hash_trait_object!` step to
//!   remember per trait.
//! - The `Hash` bound applies only to types used through the shim, not to every
//!   `MyTrait` implementor. A non-`Hash` type may still implement `MyTrait`; it
//!   simply never becomes a `DynMyTrait`. To keep the original "all implementors
//!   are `Hash`" contract, also write `trait MyTrait: Hash`.
//!
//! And nothing is lost: with the `dyn_hash` feature, `DynMyTrait` is a
//! `dyn_shim::DynHash`, so the shim still satisfies `DynHash` bounds. A `&dyn
//! DynMyTrait` upcasts to `&dyn DynHash` and flows wherever that is expected
//! (see the tail of `main`).
//!
//! The conversion: swap the import for `dyn_shim`, move `: DynHash` into the
//! attribute as `: Hash`, drop the `hash_trait_object!` call, and write `dyn
//! DynMyTrait` for the trait object.
//!
//! Run with: `cargo run --example migrate_dyn_hash` (add `--features dyn_hash` for
//! the `DynHash`-bound interop at the end).

use dyn_shim::dyn_shim;
use std::hash::{BuildHasher, BuildHasherDefault, DefaultHasher};

// Was: trait MyTrait: DynHash {
#[dyn_shim(DynMyTrait: Hash)]
trait MyTrait {
    /* ... */
}

// Was: dyn_hash::hash_trait_object!(MyTrait); (dropped, no replacement)

// Data structures containing Box<dyn DynMyTrait> can derive Hash:
#[derive(Hash)]
struct Container {
    // Was: trait_object: Box<dyn MyTrait>,
    trait_object: Box<dyn DynMyTrait>,
}

// Added to make the example runnable; the README stops at the definitions.
impl MyTrait for String {}

fn main() {
    let container = Container {
        trait_object: Box::new(String::from("jabberwock")),
    };

    // The derived Hash works even though the struct holds a trait object.
    let hash = BuildHasherDefault::<DefaultHasher>::default().hash_one(&container);
    println!("container hash: {hash:016x}");

    // Our shim still plugs into DynHash code: a Box<dyn DynMyTrait> upcasts to
    // Box<dyn DynHash>, which is `Hash` and hashes like the concrete value.
    #[cfg(feature = "dyn_hash")]
    {
        let obj: Box<dyn DynMyTrait> = Box::new(String::from("jabberwock"));
        let erased: Box<dyn dyn_shim::DynHash> = obj; // upcast
        let h = BuildHasherDefault::<DefaultHasher>::default().hash_one(&*erased);
        println!("via DynHash: {h:016x}");
    }
}
