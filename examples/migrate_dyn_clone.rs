//! Converting the `dyn_clone` README example to `#[dyn_shim]`.
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
//!         println!("{} ♫", self);
//!     }
//! }
//!
//! fn main() {
//!     let line = "The slithy structs did gyre and gimble the namespace";
//!
//!     // Build a trait object holding a String.
//!     // This requires String to implement MyTrait and std::clone::Clone.
//!     let x: Box<dyn MyTrait> = Box::new(String::from(line));
//!
//!     x.recite();
//!
//!     // The type of x2 is a Box<dyn MyTrait> cloned from x.
//!     let x2 = dyn_clone::clone_box(&*x);
//!
//!     x2.recite();
//! }
//! ```
//!
//! The conversion, applied below:
//!
//! 1. `use dyn_clone::DynClone;` becomes `use dyn_shim::dyn_shim;`.
//! 2. The `: DynClone` supertrait moves into the attribute: a plain
//!    `trait MyTrait` annotated with `#[dyn_shim(DynMyTrait: Clone)]`.
//! 3. Trait objects are written `dyn DynMyTrait` instead of `dyn MyTrait`.
//! 4. `dyn_clone::clone_box(&*x)` becomes plain `x.clone()` on the box. For
//!    a borrowed `&dyn DynMyTrait`, where `.clone()` would copy the
//!    reference, `.to_owned()` is the direct `clone_box` equivalent.
//!
//! Trait impls are untouched.
//!
//! Step 2 loosens a requirement the original had: with `: DynClone`, every
//! implementor of `MyTrait` was forced to be `Clone`. The attribute bound
//! requires `Clone` only of types used through the shim, so a non-`Clone`
//! type may now implement `MyTrait`; it simply never becomes a `DynMyTrait`
//! and cannot enter a `Box<dyn DynMyTrait>`. To keep the original contract,
//! also write the supertrait yourself: `trait MyTrait: Clone`. That composes
//! with the attribute, and gives generic code over `T: MyTrait` back its
//! `.clone()`.
//!
//! Run with: `cargo run --example migrate_dyn_clone`

use dyn_shim::dyn_shim;

// Was: trait MyTrait: DynClone {
#[dyn_shim(DynMyTrait: Clone)]
trait MyTrait {
    fn recite(&self);
}

// Unchanged.
impl MyTrait for String {
    fn recite(&self) {
        println!("{} ♫", self);
    }
}

fn main() {
    let line = "The slithy structs did gyre and gimble the namespace";

    // Build a trait object holding a String.
    // This requires String to implement MyTrait and std::clone::Clone,
    // exactly as before: the blanket impl of DynMyTrait demands both.
    // Was: let x: Box<dyn MyTrait> = ...;
    let x: Box<dyn DynMyTrait> = Box::new(String::from(line));

    x.recite();

    // The type of x2 is a Box<dyn DynMyTrait> cloned from x: the Clone bound
    // gives the box a real Clone impl, no clone_box helper needed. (When you
    // only hold a borrowed &dyn DynMyTrait, use .to_owned() instead; .clone()
    // on a reference copies the reference.)
    // Was: let x2 = dyn_clone::clone_box(&*x);
    let x2 = x.clone();

    x2.recite();
}
