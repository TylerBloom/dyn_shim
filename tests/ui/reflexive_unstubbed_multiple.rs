use dyn_shim::dyn_shim;

// When several methods cannot forward through the shim, the macro reports them
// all at once, each pointing at its own method, so they can be annotated in a
// single pass instead of one rebuild at a time.
#[dyn_shim(Dyn, reflexive = boxed)]
trait Src {
    fn get(&self) -> i32;
    fn make() -> i32;
    fn build<T>(&self, seed: T) -> i32;
}

fn main() {}
