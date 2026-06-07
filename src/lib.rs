//! Generate a dyn-compatible shim trait and blanket impl from a source trait that
//! is not dyn-compatible, forwarding only the methods you name.
//!
//! A by-value `self` method, a receiverless constructor, or a generic method
//! makes a trait not dyn-compatible, so you cannot hold a mixed set of
//! implementors behind one `Box<dyn Trait>`. [`dyn_shim!`] builds a second trait
//! containing only the dyn-compatible subset and forwards each call to the
//! original.
//!
//! ```
//! use dyn_shim::dyn_shim;
//!
//! trait Greeter {
//!     fn new() -> Self;          // receiverless: this is what makes Greeter
//!                                // not dyn-compatible, so it is left out below
//!     fn greet(&self) -> String;
//!     fn louder(&mut self);
//! }
//!
//! struct Hi(String);
//! impl Greeter for Hi {
//!     fn new() -> Self { Hi("hi".into()) }
//!     fn greet(&self) -> String { self.0.clone() }
//!     fn louder(&mut self) { self.0 = self.0.to_uppercase(); }
//! }
//!
//! // The shim wraps only the two dyn-compatible methods.
//! dyn_shim! {
//!     trait DynGreeter for Greeter {
//!         fn greet(&self) -> String;
//!         fn louder(&mut self);
//!     }
//! }
//!
//! let mut g: Box<dyn DynGreeter> = Box::new(Hi::new());
//! g.louder();
//! assert_eq!(g.greet(), "HI");
//! ```
//!
//! See [`dyn_shim!`] for the supported method forms and limitations.

/// Generate a dyn-compatible shim trait and blanket impl that forward to a source
/// trait.
///
/// # Syntax
///
/// ```ignore
/// dyn_shim! {
///     trait ShimName for SourceTrait {
///         // forwarded method signatures, each ending in `;`
///     }
/// }
/// ```
///
/// `SourceTrait` is a path, so it may be a bare name (`Src`), a module path
/// (`crate::io::Src`), or carry concrete generic arguments (`Src<u8>`). In the
/// last case the blanket impl becomes `impl<T: Src<u8>> ShimName for T`.
///
/// The forwarded signatures are written by hand; the macro does not read the
/// source trait. Each one must match the corresponding method on `SourceTrait`,
/// otherwise the generated impl fails to compile as an ordinary type error.
///
/// # What it generates
///
/// For `trait Shim for Src { ... }` the macro expands to:
///
/// ```text
/// trait Shim {
///     // one signature per method you listed
/// }
/// impl<T: Src> Shim for T {
///     // each method forwards to <T as Src>::method(self, args...)
/// }
/// ```
///
/// # Supported method forms
///
/// Each method line is a signature terminated by `;`. The following pieces are
/// recognized, in this order:
///
/// 1. **Receiver** (required), one of:
///    - `&self`
///    - `&mut self`
///    - `self: Box<Self>` — use this to forward a source method that takes
///      `self` by value. The body calls `<T as Src>::method(*self, ...)`, so
///      the call consumes the boxed object.
/// 2. **Lifetime generics** (optional): `fn f<'a>(...)`, `fn f<'a, 'b>(...)`.
/// 3. **Value arguments** (optional): any number of `name: Type` pairs.
/// 4. **Return type** (optional): `-> Type`.
/// 5. **Where clause** (optional): written in brackets as the macro's own
///    delimiter and placed last, `[where ...]`. The contents are arbitrary
///    tokens, so multi-bound clauses, commas, higher-ranked bounds
///    (`for<'x> ...`), and `Self: ...` bounds all pass through. Example:
///    `fn f<'a>(&self, x: &'a str) -> String [where 'a: 'a, Self: Sized];`
///
/// Any number of methods may be listed, including zero.
///
/// # Limitations
///
/// - **Receiverless methods are not allowed.** Associated functions with no
///   receiver, such as `fn new() -> Self` or `fn answer() -> i32`, cannot be
///   dispatched through a trait object. Omit them from the shim and call them on
///   the concrete type.
/// - **Method type-parameter generics are not allowed.** Only lifetime
///   parameters are accepted; `fn f<T>(&self, x: T)` is rejected. A method
///   generic over a type is not dyn-compatible regardless.
/// - **Only `&self`, `&mut self`, and `self: Box<Self>` are recognized.** Custom
///   receivers such as `self: Rc<Self>`, `self: Arc<Self>`, or
///   `self: Pin<&mut Self>` are not supported.
/// - **No visibility modifier.** The generated trait is always private; `pub`
///   or `pub(crate)` before `trait` is rejected. Re-export it from a module if
///   you need wider visibility.
/// - **No attributes or doc comments on method lines.** A leading `#[...]` or
///   `///` on a forwarded signature is rejected.
/// - **No `async fn`.**
/// - **No trait-level `where` clause or added supertrait bound on the shim.**
///   The source trait's bounds are already available through `T: Src` on the
///   impl, so they are not needed for forwarding. Adding a new marker bound such
///   as `: Send` to the trait object is not supported.
/// - **dyn-compatibility is not checked.** The macro is purely syntactic. A
///   method with a recognized receiver can still be dyn-incompatible, for
///   example `fn dup(&self) -> Self` (returns `Self` with no `where Self: Sized`
///   bound). The macro expands it, but using `dyn ShimName` then fails to
///   compile with error E0038.
///
/// This list is not exhaustive. The shim is itself a trait used as `dyn`, so any
/// method you forward must satisfy the language's dyn-compatibility rules. See
/// [dyn compatibility] in the Rust Reference for the full, authoritative set.
///
/// [dyn compatibility]: https://doc.rust-lang.org/reference/items/traits.html#dyn-compatibility
///
/// # Examples
///
/// A method with a lifetime parameter and a bracketed where clause:
///
/// ```
/// use dyn_shim::dyn_shim;
///
/// trait Src {
///     fn tagged<'a>(&self, tag: &'a str) -> String where 'a: 'a;
/// }
///
/// struct S;
/// impl Src for S {
///     fn tagged<'a>(&self, tag: &'a str) -> String where 'a: 'a {
///         format!("[{tag}]")
///     }
/// }
///
/// dyn_shim! {
///     trait DynSrc for Src {
///         fn tagged<'a>(&self, tag: &'a str) -> String [where 'a: 'a];
///     }
/// }
///
/// let s: &dyn DynSrc = &S;
/// assert_eq!(s.tagged("x"), "[x]");
/// ```
#[macro_export]
macro_rules! dyn_shim {
    (
        trait $shim:ident for $src:path {
            $( $methods:tt )*
        }
    ) => {
        trait $shim {
            $crate::dyn_shim!(@sig $src; $($methods)*);
        }
        impl<T: $src> $shim for T {
            $crate::dyn_shim!(@impl $src, T; $($methods)*);
        }
    };

    // ---------- signature emission ----------

    // No methods left: stop recursing.
    (@sig $src:path;) => {};
    // `&self` method: re-emit the signature, then recurse on the rest.
    (@sig $src:path;
        fn $name:ident $(<$($lt:lifetime),+>)?
            (&self $(, $arg:ident : $ty:ty)*) $(-> $ret:ty)? $([where $($w:tt)+])? ;
        $($rest:tt)*
    ) => {
        fn $name $(<$($lt),+>)? (&self $(, $arg: $ty)*) $(-> $ret)? $(where $($w)+)? ;
        $crate::dyn_shim!(@sig $src; $($rest)*);
    };
    // `&mut self` method: re-emit the signature, then recurse on the rest.
    (@sig $src:path;
        fn $name:ident $(<$($lt:lifetime),+>)?
            (&mut self $(, $arg:ident : $ty:ty)*) $(-> $ret:ty)? $([where $($w:tt)+])? ;
        $($rest:tt)*
    ) => {
        fn $name $(<$($lt),+>)? (&mut self $(, $arg: $ty)*) $(-> $ret)? $(where $($w)+)? ;
        $crate::dyn_shim!(@sig $src; $($rest)*);
    };
    // `self: Box<Self>` method: re-emit the signature, then recurse on the rest.
    (@sig $src:path;
        fn $name:ident $(<$($lt:lifetime),+>)?
            (self: Box<Self> $(, $arg:ident : $ty:ty)*) $(-> $ret:ty)? $([where $($w:tt)+])? ;
        $($rest:tt)*
    ) => {
        fn $name $(<$($lt),+>)? (self: Box<Self> $(, $arg: $ty)*) $(-> $ret)? $(where $($w)+)? ;
        $crate::dyn_shim!(@sig $src; $($rest)*);
    };

    // ---------- impl emission ----------

    // No methods left: stop recursing.
    (@impl $src:path, $T:ident;) => {};
    // `&self` method: forward to `<T as Src>::name(self, ...)`, then recurse.
    (@impl $src:path, $T:ident;
        fn $name:ident $(<$($lt:lifetime),+>)?
            (&self $(, $arg:ident : $ty:ty)*) $(-> $ret:ty)? $([where $($w:tt)+])? ;
        $($rest:tt)*
    ) => {
        fn $name $(<$($lt),+>)? (&self $(, $arg: $ty)*) $(-> $ret)? $(where $($w)+)? {
            <$T as $src>::$name(self $(, $arg)*)
        }
        $crate::dyn_shim!(@impl $src, $T; $($rest)*);
    };
    // `&mut self` method: forward to `<T as Src>::name(self, ...)`, then recurse.
    (@impl $src:path, $T:ident;
        fn $name:ident $(<$($lt:lifetime),+>)?
            (&mut self $(, $arg:ident : $ty:ty)*) $(-> $ret:ty)? $([where $($w:tt)+])? ;
        $($rest:tt)*
    ) => {
        fn $name $(<$($lt),+>)? (&mut self $(, $arg: $ty)*) $(-> $ret)? $(where $($w)+)? {
            <$T as $src>::$name(self $(, $arg)*)
        }
        $crate::dyn_shim!(@impl $src, $T; $($rest)*);
    };
    // `self: Box<Self>` method: deref the box and forward by value, then recurse.
    (@impl $src:path, $T:ident;
        fn $name:ident $(<$($lt:lifetime),+>)?
            (self: Box<Self> $(, $arg:ident : $ty:ty)*) $(-> $ret:ty)? $([where $($w:tt)+])? ;
        $($rest:tt)*
    ) => {
        fn $name $(<$($lt),+>)? (self: Box<Self> $(, $arg: $ty)*) $(-> $ret)? $(where $($w)+)? {
            <$T as $src>::$name(*self $(, $arg)*)
        }
        $crate::dyn_shim!(@impl $src, $T; $($rest)*);
    };
}
