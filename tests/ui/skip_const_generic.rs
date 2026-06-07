use dyn_shim::dyn_shim;

#[dyn_shim(Dyn)]
trait Src {
    fn keep(&self) -> i32;
    fn pick<const N: usize>(&self) -> usize; // const generic param: skipped from the shim
}

struct S;
impl Src for S {
    fn keep(&self) -> i32 {
        0
    }
    fn pick<const N: usize>(&self) -> usize {
        N
    }
}

fn main() {
    let d: &dyn Dyn = &S;
    // `pick` is not part of `Dyn`.
    let _ = d.pick::<3>();
}
