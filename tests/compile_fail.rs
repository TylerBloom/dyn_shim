// Each case applies `#[dyn_shim(Dyn)]` to a trait containing one method that is
// not dyn-compatible, then tries to reach that method through `dyn Dyn`. The
// method must be absent from the generated shim, so each program must fail to
// compile. This is the inverse check of the behavioral tests in `it.rs`.
#[test]
fn skipped_methods_absent_from_shim() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/ui/skip_receiverless.rs");
    t.compile_fail("tests/ui/skip_generic.rs");
    t.compile_fail("tests/ui/skip_async.rs");
    t.compile_fail("tests/ui/skip_self_return.rs");
    t.compile_fail("tests/ui/skip_impl_trait_arg.rs");
    t.compile_fail("tests/ui/skip_attr.rs");
}
