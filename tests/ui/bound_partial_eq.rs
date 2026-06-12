use dyn_shim::dyn_shim;

// Same rejection as `Eq`, with its own message.
#[dyn_shim(Dyn: PartialEq)]
trait Src {
    fn get(&self) -> i32;
}

fn main() {}
