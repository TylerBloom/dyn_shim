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
use syn::visit::{self, Visit};
use syn::{
    Attribute, FnArg, GenericParam, Ident, ItemTrait, Pat, ReturnType, Signature, TraitItem,
    TraitItemFn, Type, parse_macro_input,
};

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

/// Build the doc-comment lines for the generated shim trait, listing any source
/// methods that were skipped and why.
fn shim_doc(src: &Ident, skipped: &[(String, &str)]) -> Vec<String> {
    let mut lines = vec![format!("Dyn-compatible shim for [`{src}`].")];
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

/// True for a method annotated with `#[dyn_shim(skip)]`.
fn is_opted_out(method: &TraitItemFn) -> bool {
    method.attrs.iter().any(is_skip_attr)
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
