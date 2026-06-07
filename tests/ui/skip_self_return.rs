use dyn_shim::dyn_shim;

#[dyn_shim(Dyn)]
trait Src {
    fn keep(&self) -> i32;
    fn dup(&self) -> Self; // returns Self: skipped from the shim
}

struct S;
impl Src for S {
    fn keep(&self) -> i32 {
        0
    }
    fn dup(&self) -> Self {
        S
    }
}

fn main() {
    let d: &dyn Dyn = &S;
    // `dup` is not part of `Dyn`.
    let _ = d.dup();
}
