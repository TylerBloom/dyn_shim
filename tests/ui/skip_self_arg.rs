use dyn_shim::dyn_shim;

#[dyn_shim(Dyn)]
trait Src {
    fn keep(&self) -> i32;
    fn eq_self(&self, other: &Self) -> bool; // Self in argument: skipped from the shim
}

struct S;
impl Src for S {
    fn keep(&self) -> i32 {
        0
    }
    fn eq_self(&self, _other: &Self) -> bool {
        true
    }
}

fn main() {
    let d: &dyn Dyn = &S;
    // `eq_self` is not part of `Dyn`.
    let _ = d.eq_self(&S);
}
