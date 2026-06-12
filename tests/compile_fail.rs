// Each case applies `#[dyn_shim(Dyn)]` to a trait containing one method that is
// not dyn-compatible, then tries to reach that method through `dyn Dyn`. The
// method must be absent from the generated shim, so each program must fail to
// compile. This is the inverse check of the behavioral tests in `it.rs`.
#[test]
fn skipped_methods_absent_from_shim() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/ui/skip_receiverless.rs");
    t.compile_fail("tests/ui/skip_generic.rs");
    t.compile_fail("tests/ui/skip_const_generic.rs");
    t.compile_fail("tests/ui/skip_async.rs");
    t.compile_fail("tests/ui/skip_self_return.rs");
    t.compile_fail("tests/ui/skip_self_arg.rs");
    t.compile_fail("tests/ui/skip_impl_trait_arg.rs");
    t.compile_fail("tests/ui/skip_impl_trait_ret.rs");
    t.compile_fail("tests/ui/skip_self_sized.rs");
    t.compile_fail("tests/ui/skip_attr.rs");
}

// An invalid `#[dyn_shim(...)]` helper attribute is rejected with a direct
// error: on a non-method trait item the attribute is unsupported entirely, and
// on a method the only recognized argument is `skip`. Both errors come from
// the macro itself, not rustc, so the snapshots are stable across toolchains.
#[test]
fn invalid_helper_attrs_rejected() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/ui/attr_non_method.rs");
    t.compile_fail("tests/ui/attr_unknown_arg.rs");
}

// A recognized bound (`Clone`, `Hash`) constrains the blanket impl, so an
// implementor that does not satisfy it never receives the shim (a rustc
// error; the pinned toolchain keeps the snapshot stable).
#[test]
fn recognized_bounds_rejected() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/ui/bound_clone_unsatisfied.rs");
    t.compile_fail("tests/ui/bound_hash_unsatisfied.rs");
}

// Bounds the macro recognizes only to reject: each would otherwise pass
// through as a supertrait and break the shim with a confusing error far from
// the cause. An arbitrary non-dyn-compatible trait cannot be recognized by
// name, so it passes through and rustc rejects the shim at its first `dyn`
// use site (a rustc error; the pinned toolchain keeps it stable).
#[test]
fn impossible_bounds_rejected() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/ui/bound_copy.rs");
    t.compile_fail("tests/ui/bound_sized.rs");
    t.compile_fail("tests/ui/bound_default.rs");
    t.compile_fail("tests/ui/bound_eq.rs");
    t.compile_fail("tests/ui/bound_partial_eq.rs");
    t.compile_fail("tests/ui/bound_ord.rs");
    t.compile_fail("tests/ui/bound_partial_ord.rs");
    t.compile_fail("tests/ui/bound_not_dyn_compatible.rs");
    t.compile_fail("tests/ui/bound_path_form.rs");
    t.compile_fail("tests/ui/bound_maybe_sized.rs");
}

// The marker coverage of a recognized bound is opt-in: combinations exist
// only for auto traits actually listed in the bounds.
#[test]
fn unlisted_marker_not_covered() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/ui/bound_clone_unlisted_marker.rs");
}
