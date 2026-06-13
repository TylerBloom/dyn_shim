//! Generate a dyn-compatible shim trait and blanket impl from a source trait
//! that is not dyn-compatible.
//!
//! Some traits are not dyn-compatible, so you cannot hold a mixed set of
//! implementors behind one `Box<dyn Trait>`. The [`macro@dyn_shim`] attribute
//! reads the trait it is applied to, builds a second trait containing only the
//! dyn-compatible subset, and forwards each call to the original. Every
//! implementor of the source trait then works as a `dyn` shim.
//!
//! See [`macro@dyn_shim`] for the local form, [`macro@dyn_shim_foreign`] for
//! shimming a trait defined in another crate, and the method/bounds rules.
//!
//! By default the shim is a separate trait. A [reflexive
//! impl](macro@dyn_shim#reflexive-impl) (`reflexive = bare | boxed`) also makes
//! the shim's trait object satisfy the source trait itself, so it can be passed
//! to code written against the original.
//!
//! # Ready-made shims
//!
//! `Clone` and `Hash` cannot be supertraits of a dyn-compatible trait, so they
//! cannot be shimmed by restating them. This crate ships their shims directly,
//! each behind a feature, as drop-in equivalents of the `dyn-clone` and
//! `dyn-hash` crates:
//!
//! - With the `dyn_clone` feature, [`DynClone`]: `Box<dyn DynClone>` implements
//!   [`Clone`] and `dyn DynClone` implements [`ToOwned`].
//! - With the `dyn_hash` feature, [`DynHash`]: `dyn DynHash` implements [`Hash`],
//!   covering `Box<dyn DynHash>` through the standard library's forwarding impl.
//!
//! Both cover the `+ Send` and `+ Sync` marker variants. To shim `Clone` or
//! `Hash` as part of a larger trait instead, list them as
//! [bounds](macro@dyn_shim#recognized-bounds) on a `#[dyn_shim]` shim.

pub use dyn_shim_macros::{dyn_shim, dyn_shim_foreign};

// The machinery behind the ready-made shims below. It is not part of this
// crate's public API (the shims are), so it is re-exported only to define them
// and is hidden from the docs.
#[doc(hidden)]
pub use dyn_shim_macros::dyn_shim_recognized;

/// A dyn-compatible shim for [`Clone`].
///
/// Every `T: Clone` is a `DynClone`, and `Box<dyn DynClone>` is itself `Clone`,
/// cloning the underlying concrete value into a fresh box. `dyn DynClone` also
/// implements [`ToOwned`], so a borrowed `&dyn DynClone` can be promoted to an
/// owned box. The `+ Send` and `+ Sync` marker variants are covered too, so
/// `Box<dyn DynClone + Send>` is cloneable and stays `Send`.
///
/// Cloning requires `'static` contents, so `Box<dyn DynClone + 'a>` is not
/// cloneable for a borrowed `'a`.
///
/// ```
/// use dyn_shim::DynClone;
///
/// #[derive(Clone)]
/// struct Widget(u32);
///
/// let a: Box<dyn DynClone> = Box::new(Widget(7));
/// let _b = a.clone();
/// ```
#[cfg(feature = "dyn_clone")]
#[dyn_shim_recognized(Clone + Send + Sync)]
pub trait DynClone {}

/// A dyn-compatible shim for [`Hash`].
///
/// Every `T: Hash` is a `DynHash`, and `dyn DynHash` implements [`Hash`],
/// hashing like the underlying concrete value. Through the standard library's
/// `impl<T: ?Sized + Hash> Hash for Box<T>`, `Box<dyn DynHash>` and `&dyn
/// DynHash` hash the same way. The `+ Send` and `+ Sync` marker variants are
/// covered too.
///
/// ```
/// use dyn_shim::DynHash;
/// use std::hash::{BuildHasher, BuildHasherDefault, DefaultHasher};
///
/// let bh = BuildHasherDefault::<DefaultHasher>::default();
/// let boxed: Box<dyn DynHash> = Box::new(42u32);
/// assert_eq!(bh.hash_one(&*boxed), bh.hash_one(42u32));
/// ```
#[cfg(feature = "dyn_hash")]
#[dyn_shim_recognized(Hash + Send + Sync)]
pub trait DynHash {}
