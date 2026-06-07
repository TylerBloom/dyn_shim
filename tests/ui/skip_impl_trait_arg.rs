use dyn_shim::dyn_shim;
use std::fmt::Display;

#[dyn_shim(Dyn)]
trait Src {
    fn keep(&self) -> i32;
    fn sink(&self, x: impl Display); // `impl Trait` argument: skipped from the shim
}

struct S;
impl Src for S {
    fn keep(&self) -> i32 {
        0
    }
    fn sink(&self, _x: impl Display) {}
}

fn main() {
    let d: &dyn Dyn = &S;
    // `sink` is not part of `Dyn`.
    d.sink(1u8);
}
