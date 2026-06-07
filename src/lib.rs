//! Generate a dyn-compatible shim trait and blanket impl from a source trait
//! that is not dyn-compatible.
//!
//! A by-value `self` method, a receiverless constructor, or a generic method
//! makes a trait not dyn-compatible, so you cannot hold a mixed set of
//! implementors behind one `Box<dyn Trait>`. The [`macro@dyn_shim`] attribute
//! reads the trait it is applied to, builds a second trait containing only the
//! dyn-compatible subset, and forwards each call to the original.
//!
//! ```
//! use dyn_shim::dyn_shim;
//!
//! #[dyn_shim(DynGreeter)]
//! trait Greeter {
//!     fn new() -> Self;          // receiverless: skipped, not dyn-compatible
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
//! let mut g: Box<dyn DynGreeter> = Box::new(Hi::new());
//! g.louder();
//! assert_eq!(g.greet(), "HI");
//! ```
//!
//! See [`macro@dyn_shim`] for which methods are forwarded and the limitations.

use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote};
use syn::visit::{self, Visit};
use syn::{
    Attribute, FnArg, GenericParam, Ident, ItemTrait, ReturnType, Signature, TraitItem,
    TraitItemFn, Type, parse_macro_input,
};

/// Attribute macro: generate a dyn-compatible shim for the annotated trait.
///
/// # Usage
///
/// ```ignore
/// #[dyn_shim(ShimName)]
/// trait SourceTrait {
///     // methods
/// }
/// ```
///
/// The original trait is left in place. A new trait `ShimName` is generated
/// alongside it, together with `impl<T: SourceTrait> ShimName for T`, so every
/// implementor of `SourceTrait` is automatically a `ShimName` and can be used as
/// `dyn ShimName`. `ShimName` inherits the source trait's visibility.
///
/// # Which methods are forwarded
///
/// Each method is forwarded into the shim unless it cannot be dispatched through
/// a trait object. A method is **skipped** when any of the following holds:
///
/// - it has no `self` receiver (an associated function such as `fn new() -> Self`);
/// - it is `async`;
/// - it has a generic type or const parameter (lifetime parameters are fine);
/// - its return type or any argument type mentions `Self`, or uses `impl Trait`;
/// - it is annotated with `#[dyn_shim(skip)]`.
///
/// Skipped methods stay on the source trait and are reached on the concrete
/// type. Forwarded methods keep their lifetimes and `where` clause. A by-value
/// `self` receiver is rewritten to `self: Box<Self>` and forwarded by
/// dereferencing the box; `&self`, `&mut self`, and explicit boxed/pinned
/// receivers are forwarded unchanged.
///
/// # Limitations
///
/// - **The source trait may not be generic.** A trait with type, const, or
///   lifetime parameters is rejected with a compile error.
/// - **Method selection is conservative.** Any mention of `Self` in an argument
///   or return type causes a method to be skipped, even shapes that would be
///   dyn-compatible (for example a method bounded by `where Self: Sized`).
/// - **Attributes and doc comments on methods are not copied** onto the shim.
///
/// The generated shim is itself a trait used as `dyn`, so the forwarded methods
/// must satisfy the language's dyn-compatibility rules. See [Dyn Compatibility]
/// in the Rust Reference for the authoritative set.
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
    let shim_name = parse_macro_input!(attr as Ident);
    let input = parse_macro_input!(item as ItemTrait);

    if let Some(param) = input.generics.params.first() {
        return syn::Error::new_spanned(param, "dyn_shim does not support generic source traits")
            .to_compile_error()
            .into();
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
        match skip_reason(method) {
            Some(reason) => skipped.push((method.sig.ident.to_string(), reason)),
            None => {
                let (sig, body) = forward(method, src);
                sigs.push(sig);
                impls.push(body);
            }
        }
    }

    // Re-emit the source trait without our `#[dyn_shim(skip)]` helper attributes,
    // and point its docs at the generated shim.
    let mut clean = input.clone();
    for item in &mut clean.items {
        if let TraitItem::Fn(method) = item {
            method.attrs.retain(|a| !a.path().is_ident("dyn_shim"));
        }
    }
    for line in source_doc(&shim_name) {
        clean.attrs.push(syn::parse_quote! { #[doc = #line] });
    }

    let doc_attrs = shim_doc(src, &skipped)
        .into_iter()
        .map(|line| quote! { #[doc = #line] });

    quote! {
        #clean

        #(#doc_attrs)*
        #vis trait #shim_name {
            #(#sigs)*
        }

        impl<__T: #src> #shim_name for __T {
            #(#impls)*
        }
    }
    .into()
}

/// Build the doc-comment lines appended to the source trait, pointing readers at
/// the generated dyn-compatible shim.
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

/// Build the doc-comment lines for the generated shim trait, listing any source
/// methods that were skipped and why.
fn shim_doc(src: &Ident, skipped: &[(String, &str)]) -> Vec<String> {
    let mut lines = vec![format!("Dyn-compatible shim for [`{src}`].")];
    if !skipped.is_empty() {
        lines.push(String::new());
        lines.push("These methods of the source trait are not dyn-compatible, so they".to_string());
        lines.push("are not part of this shim; call them on the concrete type:".to_string());
        lines.push(String::new());
        for (name, reason) in skipped {
            lines.push(format!("- [`{src}::{name}`] ({reason})"));
        }
    }
    lines
}

/// If a method cannot be dispatched through a trait object, return a short
/// reason it is skipped. Return `None` when the method is forwarded.
fn skip_reason(method: &TraitItemFn) -> Option<&'static str> {
    let sig = &method.sig;
    if is_opted_out(method) {
        Some("opted out with #[dyn_shim(skip)]")
    } else if sig.asyncness.is_some() {
        Some("async fn")
    } else if !has_self_receiver(sig) {
        Some("no self receiver")
    } else if has_type_or_const_generics(sig) {
        Some("generic type or const parameter")
    } else if signature_mentions_self_or_impl_trait(sig) {
        Some("mentions Self or impl Trait")
    } else {
        None
    }
}

/// True for a method annotated with `#[dyn_shim(skip)]`.
fn is_opted_out(method: &TraitItemFn) -> bool {
    method.attrs.iter().any(is_skip_attr)
}

/// True if the first parameter is a `self` receiver (`&self`, `&mut self`,
/// by-value `self`, or a typed receiver such as `self: Box<Self>`).
fn has_self_receiver(sig: &Signature) -> bool {
    matches!(sig.inputs.first(), Some(FnArg::Receiver(_)))
}

/// True if the method declares a generic type or const parameter. Lifetime
/// parameters do not count, since they are forwarded as-is.
fn has_type_or_const_generics(sig: &Signature) -> bool {
    sig.generics
        .params
        .iter()
        .any(|p| !matches!(p, GenericParam::Lifetime(_)))
}

/// True if the return type or any argument type mentions `Self` or `impl Trait`.
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

/// Build the shim signature and the forwarding impl body for one method.
fn forward(method: &TraitItemFn, src: &Ident) -> (TokenStream2, TokenStream2) {
    let sig = &method.sig;
    let name = &sig.ident;
    let (generics, _, where_clause) = sig.generics.split_for_impl();
    let output = &sig.output;

    let FnArg::Receiver(recv) = sig.inputs.first().unwrap() else {
        unreachable!("should_forward guarantees a receiver")
    };
    let by_value = recv.reference.is_none() && recv.colon_token.is_none();
    let (receiver, self_expr) = if by_value {
        (quote! { self: Box<Self> }, quote! { *self })
    } else {
        (quote! { #recv }, quote! { self })
    };

    let mut decls = Vec::new();
    let mut names = Vec::new();
    for (i, arg) in sig.inputs.iter().skip(1).enumerate() {
        let FnArg::Typed(pat) = arg else { continue };
        let ty = &pat.ty;
        let id = format_ident!("__a{i}");
        decls.push(quote! { #id: #ty });
        names.push(id);
    }

    let shim_sig = quote! {
        fn #name #generics (#receiver #(, #decls)*) #output #where_clause ;
    };
    let shim_impl = quote! {
        fn #name #generics (#receiver #(, #decls)*) #output #where_clause {
            <__T as #src>::#name(#self_expr #(, #names)*)
        }
    };
    (shim_sig, shim_impl)
}

/// True for `#[dyn_shim(skip)]`.
fn is_skip_attr(attr: &Attribute) -> bool {
    if !attr.path().is_ident("dyn_shim") {
        return false;
    }
    let mut skip = false;
    let _ = attr.parse_nested_meta(|meta| {
        if meta.path.is_ident("skip") {
            skip = true;
        }
        Ok(())
    });
    skip
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
