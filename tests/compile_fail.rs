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
