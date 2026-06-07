use dyn_shim::dyn_shim;

#[dyn_shim(Dyn)]
trait Src {
    fn keep(&self) -> i32;
    #[dyn_shim(skip)]
    fn internal(&self) -> i32; // opted out: skipped from the shim
}

struct S;
impl Src for S {
    fn keep(&self) -> i32 {
        0
    }
    fn internal(&self) -> i32 {
        0
    }
}

fn main() {
    let d: &dyn Dyn = &S;
    // `internal` is not part of `Dyn`.
    let _ = d.internal();
}
