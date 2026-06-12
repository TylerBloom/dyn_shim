use dyn_shim::dyn_shim;

// `Copy` can never hold for a boxed trait object, so it is rejected up front
// instead of passing through as a dyn-compatibility-breaking supertrait.
#[dyn_shim(Dyn: Copy)]
trait Src {
    fn get(&self) -> i32;
}

fn main() {}
