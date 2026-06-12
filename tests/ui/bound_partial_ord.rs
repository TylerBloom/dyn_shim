use dyn_shim::dyn_shim;

// Same rejection as `Ord`, with its own message.
#[dyn_shim(Dyn: PartialOrd)]
trait Src {
    fn get(&self) -> i32;
}

fn main() {}
