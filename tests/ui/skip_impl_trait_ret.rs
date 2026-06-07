use dyn_shim::dyn_shim;
use std::fmt::Display;

#[dyn_shim(Dyn)]
trait Src {
    fn keep(&self) -> i32;
    fn make(&self) -> impl Display; // `impl Trait` return: skipped from the shim
}

struct S;
impl Src for S {
    fn keep(&self) -> i32 {
        0
    }
    fn make(&self) -> impl Display {
        0u8
    }
}

fn main() {
    let d: &dyn Dyn = &S;
    // `make` is not part of `Dyn`.
    let _ = d.make();
}
