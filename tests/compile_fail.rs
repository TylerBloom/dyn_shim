// Verifies the macro REJECTS dyn-incompatible method shapes at expansion,
// rather than emitting a broken shim. Each .rs in tests/ui/ must fail to compile.
#[test]
fn rejections() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/ui/by_value_self.rs");
    t.compile_fail("tests/ui/receiverless_self_return.rs");
    t.compile_fail("tests/ui/receiverless_int_return.rs");
    t.compile_fail("tests/ui/type_generic_method.rs");
}
