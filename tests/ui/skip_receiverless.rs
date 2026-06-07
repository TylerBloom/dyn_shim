use dyn_shim::dyn_shim;

#[dyn_shim(Dyn)]
trait Src {
    fn keep(&self) -> i32;
    fn answer() -> i32; // receiverless: skipped from the shim
}

struct S;
impl Src for S {
    fn keep(&self) -> i32 {
        0
    }
    fn answer() -> i32 {
        42
    }
}

fn main() {
    // `answer` is not part of `Dyn`.
    let _ = <dyn Dyn>::answer();
}
