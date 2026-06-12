//! Converting the `dyn-hash` README example to `#[dyn_shim]`.
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
//! The conversion, applied below:
//!
//! 1. `use dyn_hash::DynHash;` becomes `use dyn_shim::dyn_shim;`.
//! 2. The `: DynHash` supertrait moves into the attribute: a plain
//!    `trait MyTrait` annotated with `#[dyn_shim(DynMyTrait: Hash)]`.
//! 3. The `dyn_hash::hash_trait_object!(MyTrait);` call is dropped with no
//!    replacement; the attribute generates the `Hash` impls for the shim's
//!    `dyn` types.
//! 4. Trait objects are written `dyn DynMyTrait` instead of `dyn MyTrait`.
//!
//! Trait impls are untouched.
//!
//! Step 2 loosens a requirement the original had: with `: DynHash`, every
//! implementor of `MyTrait` was forced to be `Hash`. The attribute bound
//! requires `Hash` only of types used through the shim, so a non-`Hash` type
//! may now implement `MyTrait`; it simply never becomes a `DynMyTrait` and
//! cannot enter a `Box<dyn DynMyTrait>`. To keep the original contract, also
//! write the supertrait yourself: `trait MyTrait: Hash`. That composes with
//! the attribute, and gives generic code over `T: MyTrait` back its
//! `.hash(...)`.
//!
//! Run with: `cargo run --example migrate_dyn_hash`

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
}
