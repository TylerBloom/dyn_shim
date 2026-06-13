use dyn_shim::dyn_shim;

// `reflexive = bare + boxed` includes the bare impl, which cannot express a
// by-value `self` receiver, so the macro rejects it and points at `boxed`
// (even though the boxed half on its own would be fine).
#[dyn_shim(Dyn, reflexive = bare + boxed)]
trait Src {
    fn get(&self) -> i32;
    fn consume(self) -> i32;
}

fn main() {}
