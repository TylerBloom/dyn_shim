use dyn_shim::dyn_shim;

// `Default` has no receiver, so no vtable could ever dispatch it.
#[dyn_shim(Dyn: Default)]
trait Src {
    fn get(&self) -> i32;
}

fn main() {}
