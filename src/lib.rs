//! Generate a dyn-compatible shim trait and blanket impl from a source trait
//! that is not dyn-compatible.
//!
//! Some traits are not dyn-compatible, so you cannot hold a mixed set of
//! implementors behind one `Box<dyn Trait>`. The [`macro@dyn_shim`] attribute
//! reads the trait it is applied to, builds a second trait containing only the
//! dyn-compatible subset, and forwards each call to the original. Every
//! implementor of the source trait then works as a `dyn` shim.
//!
//! See [`macro@dyn_shim`] for an example, which methods are forwarded, and the
//! limitations.

use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote};
use syn::parse::{Parse, ParseStream};
use syn::punctuated::Punctuated;
use syn::visit::{self, Visit};
use syn::{
    Attribute, FnArg, GenericParam, Ident, ItemTrait, Pat, ReturnType, Signature, Token, TraitItem,
    TraitItemFn, Type, TypeParamBound, parse_macro_input,
};

/// Arguments to [`macro@dyn_shim`]: the shim trait's name, optionally followed
/// by supertraits to put on it, written like a trait's supertrait list
/// (`DynFoo: Send + Sync`).
struct Args {
    shim_name: Ident,
    bounds: Punctuated<TypeParamBound, Token![+]>,
}

impl Parse for Args {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let shim_name = input.parse()?;
        let mut bounds = Punctuated::new();
        if input.peek(Token![:]) {
            input.parse::<Token![:]>()?;
            bounds = Punctuated::parse_separated_nonempty(input)?;
        }
        Ok(Args { shim_name, bounds })
    }
}

/// The bound's trait path, if the bound is a plain trait name with no
/// modifier or binder. Recognition applies only to bounds written bare:
/// a modified or higher-ranked bound (`?Sized`, `for<'a> Fn(&'a str)`)
/// passes through for rustc to judge.
fn plain_trait_bound(bound: &TypeParamBound) -> Option<&syn::Path> {
    match bound {
        TypeParamBound::Trait(t)
            if matches!(t.modifier, syn::TraitBoundModifier::None) && t.lifetimes.is_none() =>
        {
            Some(&t.path)
        }
        _ => None,
    }
}

/// A std trait recognized in the bounds list. Such a trait cannot be a
/// supertrait of a dyn-compatible trait, so instead of passing it through,
/// the macro generates proxy machinery that implements it for the shim's
/// trait objects.
#[derive(Clone, Copy, PartialEq)]
enum RecognizedBound {
    Clone,
    Hash,
}

impl RecognizedBound {
    /// The absolute path this bound adds to the blanket impl: the proxy
    /// methods need the implementor to actually be `Clone`/`Hash`.
    fn impl_bound(self) -> TokenStream2 {
        match self {
            RecognizedBound::Clone => quote! { ::std::clone::Clone },
            RecognizedBound::Hash => quote! { ::std::hash::Hash },
        }
    }

    /// The line added to the generated shim's docs naming the capability, so
    /// readers of a downstream crate's docs learn it without visiting this
    /// crate's.
    fn doc_line(self, shim: &Ident) -> String {
        match self {
            RecognizedBound::Clone => format!(
                "`Box<dyn {shim}>` implements [`Clone`], and `dyn {shim}` implements \
                 [`ToOwned`], both cloning the underlying concrete value."
            ),
            RecognizedBound::Hash => format!(
                "`dyn {shim}` implements [`Hash`], hashing like the underlying \
                 concrete value."
            ),
        }
    }

    /// Generate the machinery for one recognized bound: hidden method
    /// signatures for the shim trait, their bodies for the blanket impl, and
    /// the standalone trait impls emitted after both.
    fn expand(
        self,
        shim: &Ident,
        combos: &[MarkerCombo],
    ) -> (TokenStream2, TokenStream2, TokenStream2) {
        match self {
            RecognizedBound::Clone => expand_clone(shim, combos),
            RecognizedBound::Hash => expand_hash(shim, combos),
        }
    }
}

/// A std auto trait recognized in the bounds list by its bare ident. A listed
/// auto trait passes through as a supertrait like any other bound; in
/// addition it selects which `dyn Shim + markers` types receive the machinery
/// for recognized bounds such as `Clone`, since only auto traits can follow
/// the principal trait in a trait object type.
#[derive(Clone, Copy, PartialEq)]
enum AutoTrait {
    Send,
    Sync,
    Unpin,
    UnwindSafe,
    RefUnwindSafe,
}

impl AutoTrait {
    /// The method-name suffix for a marker combination containing this trait.
    fn suffix(self) -> &'static str {
        match self {
            AutoTrait::Send => "send",
            AutoTrait::Sync => "sync",
            AutoTrait::Unpin => "unpin",
            AutoTrait::UnwindSafe => "unwind_safe",
            AutoTrait::RefUnwindSafe => "ref_unwind_safe",
        }
    }

    fn path(self) -> TokenStream2 {
        match self {
            AutoTrait::Send => quote! { ::std::marker::Send },
            AutoTrait::Sync => quote! { ::std::marker::Sync },
            AutoTrait::Unpin => quote! { ::std::marker::Unpin },
            AutoTrait::UnwindSafe => quote! { ::std::panic::UnwindSafe },
            AutoTrait::RefUnwindSafe => quote! { ::std::panic::RefUnwindSafe },
        }
    }
}

/// How one bound in the list is treated, decided by a single token match on
/// its bare name.
enum Classified {
    /// A std trait the macro implements for the shim's trait objects instead
    /// of passing it through as a supertrait (`Clone`, `Hash`).
    Recognized(RecognizedBound),
    /// An auto trait: passes through as a supertrait and additionally selects
    /// which `dyn Shim + markers` types get the recognized-bound machinery.
    Auto(AutoTrait),
    /// A std name recognized only to be rejected, carrying its targeted error
    /// message. Each would otherwise pass through as a supertrait and silently
    /// make the shim non-dyn-compatible (or is simply impossible), surfacing
    /// as a confusing rustc error far from the cause.
    Rejected(&'static str),
    /// Anything else: passed through to rustc as a supertrait.
    PassThrough,
}

/// Classify one bound by its bare name. Every recognized, auto, and rejected
/// name lives in exactly one arm here, so the categories cannot overlap and no
/// caller has to check them in a particular order. Like the literal `where
/// Self: Sized` check on methods, this is a token match: trait resolution is
/// unavailable during expansion, so a path-form bound (`std::clone::Clone`)
/// is not classified (it falls through to `PassThrough`), and a user-defined
/// trait that happens to share one of these names is treated as the std one.
fn classify(bound: &TypeParamBound) -> Classified {
    let Some(ident) = plain_trait_bound(bound).and_then(syn::Path::get_ident) else {
        return Classified::PassThrough;
    };
    match ident.to_string().as_str() {
        "Clone" => Classified::Recognized(RecognizedBound::Clone),
        "Hash" => Classified::Recognized(RecognizedBound::Hash),
        "Send" => Classified::Auto(AutoTrait::Send),
        "Sync" => Classified::Auto(AutoTrait::Sync),
        "Unpin" => Classified::Auto(AutoTrait::Unpin),
        "UnwindSafe" => Classified::Auto(AutoTrait::UnwindSafe),
        "RefUnwindSafe" => Classified::Auto(AutoTrait::RefUnwindSafe),
        "Copy" => Classified::Rejected(
            "trait objects are unsized and can never be `Copy` (use a `Clone` bound to make the shim's boxes cloneable)",
        ),
        "Sized" => Classified::Rejected(
            "trait objects are unsized, so the shim's `dyn` type can never be `Sized`",
        ),
        "Default" => Classified::Rejected(
            "`Default` has no `self` receiver and cannot be dispatched through a trait object (construct values as concrete types and box them)",
        ),
        "PartialEq" => Classified::Rejected(
            "`PartialEq` is not yet a recognized bound (cross-type equality on trait objects needs an `Any` downcast the macro does not generate)",
        ),
        "Eq" => Classified::Rejected(
            "`Eq` is not yet a recognized bound (cross-type equality on trait objects needs an `Any` downcast the macro does not generate)",
        ),
        "PartialOrd" => Classified::Rejected(
            "`PartialOrd` is not supported: the macro cannot define an order between different implementor types (sort with `sort_by_key` or implement the comparison traits for the shim's `dyn` type by hand)",
        ),
        "Ord" => Classified::Rejected(
            "`Ord` is not supported: the macro cannot define a total order between different implementor types (sort with `sort_by_key` or implement the comparison traits for the shim's `dyn` type by hand)",
        ),
        _ => Classified::PassThrough,
    }
}

/// One `dyn Shim + markers` variant the recognized-bound machinery covers: a
/// method-name suffix and the `+ ...` tokens appended to the `dyn` type.
struct MarkerCombo {
    suffix: String,
    markers: TokenStream2,
}

/// Every subset of the auto traits listed in the bounds, the plain (empty)
/// combination first. The order markers are written in a `dyn` type does not
/// affect type identity, so one impl per subset, each written in
/// the order the auto traits were listed, covers every spelling at the use
/// site. The count is `2^n` in the number of listed
/// auto traits.
fn marker_combos(autos: &[AutoTrait]) -> Vec<MarkerCombo> {
    (0..1usize << autos.len())
        .map(|mask| {
            let mut suffix = String::new();
            let mut markers = TokenStream2::new();
            for (i, auto) in autos.iter().enumerate() {
                if mask & (1 << i) == 0 {
                    continue;
                }
                suffix.push('_');
                suffix.push_str(auto.suffix());
                let path = auto.path();
                markers.extend(quote! { + #path });
            }
            MarkerCombo { suffix, markers }
        })
        .collect()
}

/// Machinery for a recognized `Clone` bound: per marker combination, a hidden
/// method cloning into a fresh box, and a `Clone` impl for that box calling
/// it. The marker has to be re-attached inside the blanket impl, where the
/// concrete type is still known; a clone erased to a plain `Box<dyn Shim>`
/// could never be coerced back to `Box<dyn Shim + Send>`. The `where Self:
/// 'static` bound licenses the `Box<__T>` to `Box<dyn Shim>` coercion in the
/// blanket impl without restricting the shim itself to `'static`
/// implementors; it holds at every call site because `Box<dyn Shim>` is `+
/// 'static` by default. (Unlike `Self: Sized`, it does not exclude the method
/// from the vtable.)
fn expand_clone(
    shim: &Ident,
    combos: &[MarkerCombo],
) -> (TokenStream2, TokenStream2, TokenStream2) {
    let mut sigs = TokenStream2::new();
    let mut impls = TokenStream2::new();
    let mut after = TokenStream2::new();
    for MarkerCombo { suffix, markers } in combos {
        let method = format_ident!("__dyn_shim_clone_box{suffix}");
        sigs.extend(quote! {
            #[doc(hidden)]
            fn #method(&self) -> ::std::boxed::Box<dyn #shim #markers>
            where
                Self: 'static #markers;
        });
        impls.extend(quote! {
            fn #method(&self) -> ::std::boxed::Box<dyn #shim #markers>
            where
                Self: 'static #markers,
            {
                ::std::boxed::Box::new(::std::clone::Clone::clone(self))
            }
        });
        // `ToOwned` rides along with `Clone`: both are facades over the same
        // hidden method, one for callers who own a box and one for callers
        // holding only `&dyn Shim` (where `.clone()` would silently copy the
        // reference). Legal because `Clone: Sized` keeps std's blanket
        // `impl<T: Clone> ToOwned for T` away from the unsized `dyn` type,
        // and `impl<T: ?Sized> Borrow<T> for Box<T>` supplies the
        // `Owned: Borrow<Self>` half of the contract.
        after.extend(quote! {
            impl ::std::clone::Clone for ::std::boxed::Box<dyn #shim #markers> {
                fn clone(&self) -> Self {
                    (**self).#method()
                }
            }

            impl ::std::borrow::ToOwned for dyn #shim #markers {
                type Owned = ::std::boxed::Box<dyn #shim #markers>;
                fn to_owned(&self) -> Self::Owned {
                    self.#method()
                }
            }
        });
    }
    (sigs, impls, after)
}

/// Machinery for a recognized `Hash` bound: one hidden method erasing the
/// generic `H: Hasher` parameter to `&mut dyn Hasher` (lossless, since std
/// implements `Hasher` for `&mut H` where `H: Hasher + ?Sized`), and a
/// `Hash` impl on each `dyn` type calling it. Implementing on the `dyn`
/// types directly means std's `impl<T: ?Sized + Hash> Hash for Box<T>`
/// forwards for free and `&dyn Shim` is covered too. Hashing only reads, so
/// one hidden method serves every marker combination.
fn expand_hash(shim: &Ident, combos: &[MarkerCombo]) -> (TokenStream2, TokenStream2, TokenStream2) {
    let sigs = quote! {
        #[doc(hidden)]
        fn __dyn_shim_hash(&self, state: &mut dyn ::std::hash::Hasher);
    };
    let impls = quote! {
        fn __dyn_shim_hash(&self, mut state: &mut dyn ::std::hash::Hasher) {
            <__T as ::std::hash::Hash>::hash(self, &mut state)
        }
    };
    let mut after = TokenStream2::new();
    for MarkerCombo { markers, .. } in combos {
        after.extend(quote! {
            impl ::std::hash::Hash for dyn #shim #markers {
                fn hash<__H: ::std::hash::Hasher>(&self, state: &mut __H) {
                    self.__dyn_shim_hash(state)
                }
            }
        });
    }
    (sigs, impls, after)
}

/// Generate a dyn-compatible shim for the annotated trait.
///
/// # Usage
///
/// ```
/// use dyn_shim::dyn_shim;
///
/// #[dyn_shim(DynFoo)]
/// trait Foo {
///     fn describe(&self) -> String;
///
///     fn make() -> Self;        // skipped: receiverless, not dyn-compatible
///
///     #[dyn_shim(skip)]
///     fn debug_only(&self) {}   // skipped: opted out
/// }
/// ```
///
/// The original trait is left in place. A new trait `DynFoo` is generated
/// alongside it, together with `impl<T: Foo> DynFoo for T`, so every
/// implementor of `Foo` is automatically a `DynFoo` and can be used as `dyn
/// DynFoo`. `DynFoo` inherits the source trait's visibility.
///
/// # Method Selection
///
/// A method is forwarded into the shim only if it can be dispatched through a
/// trait object. The criteria below approximate the language's [Dyn
/// Compatibility] rules per method. They catch the common reasons a method is
/// not callable on a `dyn` type, but do not reproduce the full rule set. A
/// method is **skipped** when any of the following holds:
///
/// - It has no `self` receiver (an associated function such as `fn new() -> Self`).
/// - It is `async`.
/// - It has a generic type or const parameter (lifetime parameters are fine).
/// - Its return type or any argument type mentions `Self`, or uses `impl Trait`.
/// - It requires `Self: Sized` (such a method is excluded from the vtable).
/// - It is annotated with `#[dyn_shim(skip)]`.
///
/// Skipped methods stay on the source trait and are reached on the concrete
/// type. A forwarded method keeps its entire signature — lifetimes, `where`
/// clause, parameter names, `unsafe`, and any explicit ABI — as well as its
/// attributes, so `#[doc]`, `#[must_use]`, `#[deprecated]`, and `#[cfg]`
/// behave the same on the shim as on the source trait. A by-value
/// `self` receiver is rewritten to `self: Box<Self>` and forwarded by
/// dereferencing the box. A dispatchable receiver (`&self`, `&mut self`, or an
/// explicit `self: Box<Self>`, `Rc<Self>`, `Arc<Self>`, or `Pin<_>`) is
/// forwarded unchanged.
///
/// # Bounds
///
/// The generated shim has no supertraits by default — not even the source
/// trait's. Optional bounds after the shim's name, written like a trait's
/// supertrait list, are added as supertraits of the shim and as bounds on the
/// blanket impl:
///
/// ```
/// use dyn_shim::dyn_shim;
///
/// #[dyn_shim(DynJob: Send + Sync)]
/// trait Job {
///     fn run(&self) -> u32;
/// }
///
/// struct Sleep;
/// impl Job for Sleep {
///     fn run(&self) -> u32 { 1 }
/// }
///
/// let job: Box<dyn DynJob> = Box::new(Sleep);
/// assert_eq!(std::thread::spawn(move || job.run()).join().unwrap(), 1);
/// ```
///
/// This is also the way to re-add a supertrait of the source trait, making its
/// methods callable on the shim's `dyn` type (`DynShim: std::fmt::Display`,
/// for example).
///
/// The shim's bounds should generally mirror the source trait's supertraits,
/// keeping the shim a faithful dyn-compatible view of the source. Auto
/// traits are the common exception: a `Send` or `Sync` bound describes
/// implementors rather than the trait's contract, so it usually appears only
/// on the shim, as above.
///
/// A bound the source trait does not require deserves a warning, because it
/// does not behave like the supertrait it is spelled as:
///
/// ```
/// use dyn_shim::dyn_shim;
///
/// #[dyn_shim(DynFoo: Iterator)]
/// trait Foo {
///     fn describe(&self) -> String;
/// }
///
/// // Implements Foo, but not Iterator.
/// struct Bar;
/// impl Foo for Bar {
///     fn describe(&self) -> String { "bar".into() }
/// }
///
/// // Compiles: implementing Foo carries no Iterator obligation. Bar just
/// // silently never receives the DynFoo blanket impl, so this would fail:
/// // let b: Box<dyn DynFoo<Item = u8>> = Box::new(Bar);
/// ```
///
/// Had `trait DynFoo: Iterator` been written by hand, each
/// `impl DynFoo for ...` would be checked against the supertrait, making an
/// implementor without `Iterator` an error at its `impl`. But nobody writes
/// impls of the shim. There is only the blanket impl, and a bound there is a
/// filter, not an obligation: `Bar` implements `Foo` fine, never becomes a
/// `DynFoo`, and errors only where it is used as `Box<dyn DynFoo>`, which
/// may be far from the mistake, or nowhere. Mirroring the bound as a
/// supertrait of the source (`trait Foo: Iterator`) restores the immediate
/// per-impl check.
///
/// **The macro cannot classify the listed bounds.** Trait resolution is
/// unavailable during macro expansion, so whether a named trait is
/// dyn-compatible cannot be decided there. Auto traits such as `Send` and
/// `Sync`, lifetimes such as `'static`, and dyn-compatible traits pass
/// through and work as supertraits. A few std names are handled specially by
/// token match: [recognized](#recognized-bounds) ones get generated
/// machinery, and [known-impossible](#rejected-bounds) ones get a targeted
/// error. Anything else that breaks the shim (a non-dyn-compatible user
/// trait, a path-form `std::clone::Clone`) makes the shim non-dyn-compatible
/// too; rustc reports that at the first place `dyn Shim` is written, not at
/// the attribute. An implementor of the source trait that does not satisfy
/// the bounds does not receive the shim impl.
///
/// ## Recognized Bounds
///
/// `Clone` and `Hash` are exceptions to the rule above. Neither can be a
/// supertrait of a dyn-compatible trait, so a literal `Clone` or `Hash` in
/// the bounds list is intercepted instead of passed through. It still bounds
/// the blanket impl (an implementor that is not `Clone` does not receive the
/// shim), and instead of a supertrait the macro generates the machinery that
/// implements the trait for the shim's trait objects:
///
/// ```
/// use dyn_shim::dyn_shim;
/// use std::hash::{DefaultHasher, Hash, Hasher};
///
/// #[dyn_shim(DynShape: Clone + Hash)]
/// trait Shape {
///     fn area(&self) -> u32;
/// }
///
/// #[derive(Clone, Hash)]
/// struct Rect(u32, u32);
/// impl Shape for Rect {
///     fn area(&self) -> u32 { self.0 * self.1 }
/// }
///
/// let shapes: Vec<Box<dyn DynShape>> = vec![Box::new(Rect(2, 3))];
/// let copy = shapes.clone(); // Box<dyn DynShape>: Clone
/// assert_eq!(copy[0].area(), 6);
///
/// let mut hasher = DefaultHasher::new();
/// shapes[0].hash(&mut hasher); // Box<dyn DynShape>: Hash
///
/// let borrowed: &dyn DynShape = &Rect(2, 3);
/// let owned: Box<dyn DynShape> = borrowed.to_owned(); // dyn DynShape: ToOwned
/// assert_eq!(owned.area(), 6);
/// ```
///
/// `Clone` is implemented for `Box<dyn Shim>`. `Hash` is implemented for
/// `dyn Shim` itself, which also covers `&dyn Shim` and, through std's
/// forwarding impl, `Box<dyn Shim>`. Cloning requires `'static` contents:
/// `Box<dyn Shim + 'a>` does not get `Clone`.
///
/// A recognized `Clone` also implements `ToOwned` for the `dyn` type. The
/// two are facades over the same machinery serving different callers:
/// `Clone` duplicates a box you already own, while `to_owned` lets a caller
/// holding only a borrowed `&dyn Shim` escape the borrow with an owned copy.
/// That borrowed edge is otherwise a footgun, since `&T` is itself `Clone`:
/// `shape_ref.clone()` compiles and silently copies the reference, not the
/// value. `ToOwned` is also what `Cow<'_, dyn Shim>` requires, enabling APIs
/// that pass borrowed values through untouched and allocate only on the
/// owning path.
///
/// Auto traits listed in the bounds select which marker types are covered as
/// well. For each subset of the listed auto traits (`Send`, `Sync`, `Unpin`,
/// `UnwindSafe`, and `RefUnwindSafe` are recognized), the trait is also
/// implemented for `Box<dyn Shim + markers>` (`dyn Shim + markers` for
/// `Hash`): `Clone + Send` covers `Box<dyn Shim>` and `Box<dyn Shim + Send>`.
/// A listed auto trait otherwise behaves as before, becoming a supertrait of
/// the shim and a bound on the blanket impl. Position in the bounds list
/// never matters, nor does marker order at the use site (`Box<dyn Send +
/// Shim>` is the same type). The number of generated impls doubles with each
/// listed auto trait.
///
/// Like the literal `where Self: Sized` check on methods, recognition is a
/// token match on the bare name: a path-form `std::clone::Clone` is passed
/// through as a supertrait (breaking the shim's dyn-compatibility), and a
/// user-defined trait named `Clone` is intercepted. Trait resolution is
/// unavailable during expansion, so the macro cannot see what a name is
/// imported as; the bare ident is all it has.
///
/// The same applies to the auto traits that select marker combinations. Only
/// a bare `Send` (or `Sync`, `Unpin`, `UnwindSafe`, `RefUnwindSafe`) is added
/// to the covered subsets. A path-form `std::marker::Send` still passes
/// through as a supertrait, so the bound itself compiles, but it is left out
/// of the marker machinery, so `Box<dyn Shim + Send>` never receives the
/// `Clone` or `Hash` impl. The miss surfaces as a trait-bound error at the
/// `Box<dyn Shim + Send>` use site, not at the attribute. Write auto traits
/// bare when they should drive the marker combinations.
///
/// The generated machinery names the shim's `dyn` type bare (`Box<dyn
/// Shim>`), which requires every associated type of every supertrait to be
/// fixed. So a recognized bound combines with a bound whose trait has
/// associated types (such as `Iterator`) only when the bounds list binds
/// them: `Clone + Iterator<Item = u8>` works, while `Clone + Iterator` does
/// not, since an unbound `Item` could only be supplied at a use site, which
/// the generated impls never see.
///
/// ## Rejected Bounds
///
/// Some std names are recognized only to be rejected with a targeted error,
/// because no machinery could make them work: `Copy` and `Sized` contradict
/// being a trait object, and `Default` has no receiver for a vtable to
/// dispatch on. The comparison traits are rejected too: `PartialEq` and `Eq`
/// need an `Any` downcast the macro does not generate yet, and `PartialOrd`
/// and `Ord` would make the macro invent an order between unrelated concrete
/// types, which is not its call. Sort with `sort_by_key`, or implement the
/// comparison traits on the `dyn` type by hand: the crate invoking the macro
/// owns the shim trait, so coherence permits `impl Ord for dyn Shim` there
/// (std's forwarding impls carry it onto the boxes), and the generated
/// machinery stays out of the way.
///
/// Rejection is a bare-name token match, with the same blind spot as
/// recognition: the macro cannot see what a name resolves to. A user-defined
/// trait that happens to be named `Ord` (or any of the rejected names) is
/// caught too, and reported with the message written for the std trait, even
/// when that trait is dyn-compatible and would work as a supertrait. Write it
/// path-qualified (`self::Ord`, `crate::cmp::Ord`) to pass it through: a
/// multi-segment path is not a bare ident, so it skips the rejection list and
/// becomes an ordinary supertrait.
///
/// ## Bounds That Need No Entry
///
/// A dyn-compatible trait works as a plain pass-through bound; nothing needs
/// recognizing. This covers, among many others, `Debug`, `Display`,
/// `std::error::Error`, the auto traits, `AsRef<T>` and `Borrow<T>`,
/// `std::io::Read`/`Write`/`Seek`, `Iterator`, and `Future`. The last two
/// carry associated types, which need one extra step; see
/// [Bounds With Associated Types](#bounds-with-associated-types).
///
/// `Any` is worth singling out: trait object upcasting is built into the
/// language, so an `Any` bound already enables downcasting with no generated
/// machinery:
///
/// ```
/// use dyn_shim::dyn_shim;
/// use std::any::Any;
///
/// #[dyn_shim(DynShape: Any)]
/// trait Shape {
///     fn area(&self) -> u32;
/// }
///
/// struct Rect(u32, u32);
/// impl Shape for Rect {
///     fn area(&self) -> u32 { self.0 * self.1 }
/// }
///
/// let shape: Box<dyn DynShape> = Box::new(Rect(2, 3));
/// let any: &dyn Any = &*shape; // upcasting coercion
/// assert!(any.downcast_ref::<Rect>().is_some());
/// ```
///
/// ## Bounds With Associated Types
///
/// A bound whose trait has associated types, such as `Iterator` or
/// `Future`, passes through like any other dyn-compatible trait, but the
/// associated types must be bound before the shim's `dyn` type can be
/// written. There are two places to bind them, and they trade against each
/// other:
///
/// - **At the use site.** The bounds list names the bare trait
///   (`DynSamples: Iterator`), and every spot that writes the `dyn` type
///   supplies the bindings: `Box<dyn DynSamples<Item = u8>>`. One shim
///   serves every item type, each collection picking its own binding, but
///   the bare `dyn DynSamples` is not a nameable type (forgetting the
///   binding is a "must be specified" error at that spot).
/// - **In the bounds list.** `DynSamples: Iterator<Item = u8>` fixes the
///   associated type for every implementor, so the bare `dyn DynSamples`
///   is nameable. Only implementors with exactly that item type receive
///   the shim, and this is the only form that combines with
///   [recognized bounds](#recognized-bounds), whose generated machinery
///   must name the `dyn` type bare.
///
/// ```
/// use dyn_shim::dyn_shim;
///
/// #[dyn_shim(DynSamples: Iterator)]
/// trait Samples: Iterator {
///     fn label(&self) -> String;
/// }
///
/// struct Ramp(u8);
/// impl Iterator for Ramp {
///     type Item = u8;
///     fn next(&mut self) -> Option<u8> {
///         self.0 += 1;
///         Some(self.0)
///     }
/// }
/// impl Samples for Ramp {
///     fn label(&self) -> String { "ramp".into() }
/// }
///
/// let mut source: Box<dyn DynSamples<Item = u8>> = Box::new(Ramp(0));
/// assert_eq!(source.label(), "ramp");
/// let head: Vec<u8> = source.by_ref().take(3).collect();
/// assert_eq!(head, [1, 2, 3]);
/// ```
///
/// # Limitations
///
/// A skipped method (see [Method Selection](#method-selection)) is not a
/// limitation of this macro: it cannot be dispatched through any trait object,
/// so no shim could forward it. The limitations specific to this macro are:
///
/// - **The source trait may not be generic.** A trait with type, const, or
///   lifetime parameters is rejected with a compile error. Such a trait can
///   still be dyn-compatible on its own (`dyn Trait<i32>`); the macro just does
///   not generate a parameterized shim for it.
/// - **Supertraits are not inherited.** The macro cannot tell whether a
///   supertrait is dyn-compatible, so it carries none of them onto the shim and
///   their methods are not callable on the shim's `dyn` type. Re-add the ones
///   you need — and know to be dyn-compatible — as [bounds on the shim's
///   name](#bounds).
/// - **Only a literal `where Self: Sized` bound is recognized.** Classifying any
///   other `Self:` bound would need trait resolution, which is unavailable during
///   macro expansion, so such a method is forwarded as written. This is correct
///   for an auto-trait bound like `Self: Send` (call it through `&(dyn Shim +
///   Send)`), but a `Self: Clone` bound produces a method that cannot be called
///   on the shim's `dyn` type, and a `Self: Debug` bound produces a shim that
///   does not compile. Annotate such a method with `#[dyn_shim(skip)]`.
///
/// [Dyn Compatibility]: https://doc.rust-lang.org/reference/items/traits.html#dyn-compatibility
///
/// # Example
///
/// ```
/// use dyn_shim::dyn_shim;
///
/// #[dyn_shim(DynSink)]
/// trait Sink {
///     fn connect() -> Self;                 // skipped: receiverless
///     fn write(&mut self, line: &str);
///     fn total(&self) -> usize;
///     fn finish(self) -> usize;             // by-value -> self: Box<Self>
///     #[dyn_shim(skip)]
///     fn debug_only(&self) {}               // skipped: opted out
/// }
///
/// #[derive(Default)]
/// struct Buf { lines: usize }
/// impl Sink for Buf {
///     fn connect() -> Self { Buf::default() }
///     fn write(&mut self, _line: &str) { self.lines += 1; }
///     fn total(&self) -> usize { self.lines }
///     fn finish(self) -> usize { self.lines }
/// }
///
/// let mut s: Box<dyn DynSink> = Box::new(Buf::connect());
/// s.write("a");
/// s.write("b");
/// assert_eq!(s.total(), 2);
/// assert_eq!(s.finish(), 2);
/// ```
#[proc_macro_attribute]
pub fn dyn_shim(attr: TokenStream, item: TokenStream) -> TokenStream {
    let Args { shim_name, bounds } = parse_macro_input!(attr as Args);
    let input = parse_macro_input!(item as ItemTrait);

    if let Some(param) = input.generics.params.first() {
        return syn::Error::new_spanned(param, "dyn_shim does not support generic source traits")
            .to_compile_error()
            .into();
    }

    // Partition the bounds list. A recognized std trait (`Clone`, `Hash`) is
    // drained: as a supertrait it would break dyn-compatibility, so it
    // instead becomes a bound on the blanket impl plus proxy machinery on the
    // shim's trait objects. A recognized auto trait passes through like any
    // other bound and additionally selects which `dyn Shim + markers` types
    // get that machinery. Position in the list never matters.
    let mut recognized = Vec::new();
    let mut autos = Vec::new();
    let mut passthrough: Punctuated<TypeParamBound, Token![+]> = Punctuated::new();
    for bound in bounds {
        // Duplicates are deduplicated silently, matching the language's own
        // tolerance of `trait Foo: A + A`.
        match classify(&bound) {
            Classified::Rejected(msg) => {
                return syn::Error::new_spanned(&bound, msg)
                    .to_compile_error()
                    .into();
            }
            Classified::Recognized(k) => {
                if !recognized.contains(&k) {
                    recognized.push(k);
                }
            }
            Classified::Auto(auto) => {
                if !autos.contains(&auto) {
                    autos.push(auto);
                }
                passthrough.push(bound);
            }
            Classified::PassThrough => passthrough.push(bound),
        }
    }
    // The marker combinations only feed the recognized-bound machinery, so
    // there is nothing to compute when no recognized bound is present.
    let combos = if recognized.is_empty() {
        Vec::new()
    } else {
        marker_combos(&autos)
    };

    // Validate the `#[dyn_shim(...)]` helper attributes: on a method the only
    // supported argument is `skip`; on any other trait item the attribute is
    // rejected outright. Only methods are stripped of it before the trait is
    // re-emitted, so left in place rustc would re-expand it as this attribute
    // macro and fail with an unrelated parse error pointing at the item.
    for item in &input.items {
        let attrs = match item {
            TraitItem::Fn(item) => {
                for attr in item.attrs.iter().filter(|a| a.path().is_ident("dyn_shim")) {
                    if let Err(err) = require_skip(attr) {
                        return err.to_compile_error().into();
                    }
                }
                continue;
            }
            TraitItem::Const(item) => &item.attrs,
            TraitItem::Type(item) => &item.attrs,
            TraitItem::Macro(item) => &item.attrs,
            _ => continue,
        };
        if let Some(attr) = attrs.iter().find(|a| a.path().is_ident("dyn_shim")) {
            return syn::Error::new_spanned(
                attr,
                "#[dyn_shim] attributes are only supported on methods",
            )
            .to_compile_error()
            .into();
        }
    }

    let src = &input.ident;
    let vis = &input.vis;

    let mut sigs = Vec::new();
    let mut impls = Vec::new();
    let mut skipped: Vec<(String, &str)> = Vec::new();
    for item in &input.items {
        let TraitItem::Fn(method) = item else {
            continue;
        };
        match skip(method) {
            Some(reason) => skipped.push((method.sig.ident.to_string(), reason)),
            None => {
                let (sig, body) = forward(method, src);
                sigs.push(sig);
                impls.push(body);
            }
        }
    }

    // Re-emit the source trait without our `#[dyn_shim(skip)]` helper
    // attributes, and point its docs at the generated shim.
    let mut clean = input.clone();
    for item in &mut clean.items {
        if let TraitItem::Fn(method) = item {
            method.attrs.retain(|a| !a.path().is_ident("dyn_shim"));
        }
    }
    for line in source_doc(&shim_name) {
        clean.attrs.push(syn::parse_quote! { #[doc = #line] });
    }

    let doc_attrs = shim_doc(src, &shim_name, &recognized, &skipped)
        .into_iter()
        .map(|line| quote! { #[doc = #line] });

    // The passed-through bounds become the shim's supertraits, so the blanket
    // impl must require them of the implementor as well. A recognized bound
    // requires its trait of the implementor too, but only on the impl.
    let supertraits = (!passthrough.is_empty()).then(|| quote! { : #passthrough });
    let impl_bounds = (!passthrough.is_empty()).then(|| quote! { + #passthrough });
    let recognized_bounds: TokenStream2 = recognized
        .iter()
        .map(|k| {
            let path = k.impl_bound();
            quote! { + #path }
        })
        .collect();

    let mut recognized_sigs = TokenStream2::new();
    let mut recognized_impls = TokenStream2::new();
    let mut recognized_extra = TokenStream2::new();
    for k in &recognized {
        let (sigs, impls, extra) = k.expand(&shim_name, &combos);
        recognized_sigs.extend(sigs);
        recognized_impls.extend(impls);
        recognized_extra.extend(extra);
    }

    quote! {
        #clean

        #(#doc_attrs)*
        #vis trait #shim_name #supertraits {
            #(#sigs)*
            #recognized_sigs
        }

        impl<__T: #src #impl_bounds #recognized_bounds> #shim_name for __T {
            #(#impls)*
            #recognized_impls
        }

        #recognized_extra
    }
    .into()
}

/// Build the shim signature and the forwarding impl body for one method.
///
/// The shim method reuses the source method's entire signature (`unsafe`, ABI,
/// generics, `where` clause, ...) and its attributes, rewriting only the
/// inputs: a by-value `self` becomes `self: Box<Self>`, and each argument
/// keeps its declared name where it has one. Copying the attributes keeps
/// `#[doc]`, `#[must_use]`, and `#[deprecated]` working on the shim, and keeps
/// a `#[cfg]`-gated method gated consistently across the source trait, the
/// shim trait, and the blanket impl.
fn forward(method: &TraitItemFn, src: &Ident) -> (TokenStream2, TokenStream2) {
    let mut sig = method.sig.clone();

    let Some(FnArg::Receiver(recv)) = sig.inputs.first() else {
        unreachable!("skip guarantees a receiver")
    };
    // `self: Self` is the explicit spelling of by-value `self`; only a typed
    // receiver with a real wrapper type (Box, Rc, Arc, Pin, ...) is forwarded
    // unchanged.
    let by_value = recv.reference.is_none()
        && (recv.colon_token.is_none()
            || matches!(&*recv.ty, Type::Path(p) if p.qself.is_none() && p.path.is_ident("Self")));
    let self_expr = if by_value {
        // Absolute path: the expansion must not depend on what `Box` names at
        // the call site (a local shadow, or a missing prelude under no_std).
        sig.inputs[0] = syn::parse_quote! { self: ::std::boxed::Box<Self> };
        quote! { *self }
    } else {
        quote! { self }
    };

    let mut names = Vec::new();
    for (i, arg) in sig.inputs.iter_mut().skip(1).enumerate() {
        let FnArg::Typed(pat) = arg else { continue };
        // Keep the declared name; a non-trivial pattern (only legal on a
        // defaulted method) gets a synthetic name the impl can forward.
        let id = match &*pat.pat {
            Pat::Ident(p) if p.by_ref.is_none() && p.subpat.is_none() => p.ident.clone(),
            _ => format_ident!("__a{i}"),
        };
        *pat.pat = syn::parse_quote! { #id };
        names.push(id);
    }

    let attrs: Vec<&Attribute> = method
        .attrs
        .iter()
        .filter(|a| !a.path().is_ident("dyn_shim"))
        .collect();
    // The impl method only takes the `cfg` gates: attributes like `#[must_use]`
    // and `#[deprecated]` are rejected on trait methods in impl blocks (which
    // also rules out forwarding `cfg_attr`, since it can expand to them), but a
    // `#[cfg]`-gated method must stay gated everywhere it is emitted.
    // `#[allow]` keeps the generated forwarding call to a `#[deprecated]`
    // method from warning.
    let cfg_attrs: Vec<&Attribute> = attrs
        .iter()
        .copied()
        .filter(|a| a.path().is_ident("cfg"))
        .collect();

    let name = &sig.ident;
    let shim_sig = quote! {
        #(#attrs)*
        #sig ;
    };
    let shim_impl = quote! {
        #(#cfg_attrs)*
        #[allow(deprecated)]
        #sig {
            <__T as #src>::#name(#self_expr #(, #names)*)
        }
    };
    (shim_sig, shim_impl)
}

/// Build the doc-comment lines appended to the source trait, pointing readers
/// at the generated dyn-compatible shim.
fn source_doc(shim_name: &Ident) -> Vec<String> {
    vec![
        String::new(),
        "# Dyn Compatibility".to_string(),
        String::new(),
        format!(
            "[`{shim_name}`] is a generated dyn-compatible shim for this trait. \
             Use `dyn {shim_name}` to hold implementors behind a trait object."
        ),
    ]
}

/// Build the doc-comment lines for the generated shim trait: the capabilities
/// added by recognized bounds, and any source methods that were skipped and
/// why.
fn shim_doc(
    src: &Ident,
    shim: &Ident,
    recognized: &[RecognizedBound],
    skipped: &[(String, &str)],
) -> Vec<String> {
    let mut lines = vec![format!("Dyn-compatible shim for [`{src}`].")];
    if !recognized.is_empty() {
        lines.push(String::new());
        for k in recognized {
            lines.push(k.doc_line(shim));
        }
    }
    if !skipped.is_empty() {
        lines.push(String::new());
        lines.push("These methods of the source trait are not dyn-compatible, so they".to_string());
        lines.push("are not part of this shim. Call them on the concrete type.".to_string());
        lines.push(String::new());
        for (name, reason) in skipped {
            lines.push(format!("- [`{src}::{name}`] ({reason})"));
        }
    }
    lines
}

/// If a method cannot be dispatched through a trait object, return a short
/// reason it is skipped. Return `None` when the method is forwarded.
fn skip(method: &TraitItemFn) -> Option<&'static str> {
    let sig = &method.sig;
    if is_opted_out(method) {
        Some("opted out with #[dyn_shim(skip)]")
    } else if sig.asyncness.is_some() {
        Some("async fn")
    } else if !has_self_receiver(sig) {
        Some("no self receiver")
    } else if has_type_or_const_generics(sig) {
        Some("generic type or const parameter")
    } else if requires_self_sized(sig) {
        Some("requires Self: Sized")
    } else if signature_mentions_self_or_impl_trait(sig) {
        Some("mentions Self or impl Trait")
    } else {
        None
    }
}

/// Require a method's `#[dyn_shim(...)]` attribute to be exactly
/// `#[dyn_shim(skip)]`, the only supported helper argument.
fn require_skip(attr: &Attribute) -> syn::Result<()> {
    let mut skip = false;
    attr.parse_nested_meta(|meta| {
        if meta.path.is_ident("skip") {
            skip = true;
            Ok(())
        } else {
            Err(meta.error("unsupported dyn_shim argument, expected `skip`"))
        }
    })?;
    if skip {
        Ok(())
    } else {
        Err(syn::Error::new_spanned(attr, "expected #[dyn_shim(skip)]"))
    }
}

/// True for a method annotated with `#[dyn_shim(skip)]`. The attribute's
/// arguments were validated up front, so its presence alone means skip.
fn is_opted_out(method: &TraitItemFn) -> bool {
    method.attrs.iter().any(|a| a.path().is_ident("dyn_shim"))
}

/// True if the first parameter is a `self` receiver (`&self`, `&mut self`,
/// by-value `self`, or a typed receiver such as `self: Box<Self>`).
fn has_self_receiver(sig: &Signature) -> bool {
    matches!(sig.inputs.first(), Some(FnArg::Receiver(_)))
}

/// True if the method's `where` clause requires `Self: Sized`. Such a method is
/// excluded from the vtable, so it cannot be dispatched through the shim's
/// `dyn` type even though its signature is otherwise compatible.
fn requires_self_sized(sig: &Signature) -> bool {
    let Some(where_clause) = &sig.generics.where_clause else {
        return false;
    };
    where_clause.predicates.iter().any(|pred| {
        let syn::WherePredicate::Type(pred) = pred else {
            return false;
        };
        let Type::Path(bounded) = &pred.bounded_ty else {
            return false;
        };
        if bounded.qself.is_some() || !bounded.path.is_ident("Self") {
            return false;
        }
        pred.bounds
            .iter()
            .any(|bound| matches!(bound, syn::TypeParamBound::Trait(t) if t.path.is_ident("Sized")))
    })
}

/// True if the method declares a generic type or const parameter. Lifetime
/// parameters do not count, since they are forwarded as-is.
fn has_type_or_const_generics(sig: &Signature) -> bool {
    sig.generics
        .params
        .iter()
        .any(|p| !matches!(p, GenericParam::Lifetime(_)))
}

/// True if the return type or any argument type mentions `Self` or `impl
/// Trait`.
fn signature_mentions_self_or_impl_trait(sig: &Signature) -> bool {
    let return_bad =
        matches!(&sig.output, ReturnType::Type(_, ty) if mentions_self_or_impl_trait(ty));

    let arg_bad = sig
        .inputs
        .iter()
        .skip(1)
        .any(|arg| matches!(arg, FnArg::Typed(pat) if mentions_self_or_impl_trait(&pat.ty)));

    return_bad || arg_bad
}

/// True if a type mentions `Self` or uses `impl Trait`, either of which makes a
/// method non-dyn-compatible.
fn mentions_self_or_impl_trait(ty: &Type) -> bool {
    struct Finder(bool);
    impl<'ast> Visit<'ast> for Finder {
        fn visit_path(&mut self, path: &'ast syn::Path) {
            if path.segments.iter().any(|s| s.ident == "Self") {
                self.0 = true;
            }
            visit::visit_path(self, path);
        }
        fn visit_type_impl_trait(&mut self, it: &'ast syn::TypeImplTrait) {
            self.0 = true;
            visit::visit_type_impl_trait(self, it);
        }
    }
    let mut finder = Finder(false);
    finder.visit_type(ty);
    finder.0
}
