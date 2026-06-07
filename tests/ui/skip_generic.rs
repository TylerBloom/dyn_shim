use dyn_shim::dyn_shim;

#[dyn_shim(Dyn)]
trait Src {
    fn keep(&self) -> i32;
    fn g<U>(&self, x: U); // generic type param: skipped from the shim
}

struct S;
impl Src for S {
    fn keep(&self) -> i32 {
        0
    }
    fn g<U>(&self, _x: U) {}
}

fn main() {
    let d: &dyn Dyn = &S;
    // `g` is not part of `Dyn`.
    d.g(1u8);
}
