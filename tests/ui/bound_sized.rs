use dyn_shim::dyn_shim;

// A `Sized` bound contradicts producing a trait object at all.
#[dyn_shim(Dyn: Sized)]
trait Src {
    fn get(&self) -> i32;
}

fn main() {}
