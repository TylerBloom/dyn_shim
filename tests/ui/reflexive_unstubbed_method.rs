use dyn_shim::dyn_shim;

// The reflexive `impl Src for Box<dyn Dyn>` must provide every method of
// `Src`, but a method that is not dyn-compatible cannot forward through the
// shim. Such a method must be opted into a panicking stub with
// `#[dyn_shim(panic)]`; leaving it unannotated is an error rather than a
// silently incomplete impl.
#[dyn_shim(Dyn, reflexive = boxed)]
trait Src {
    fn get(&self) -> i32;
    fn build<T>(&self, seed: T) -> i32;
}

fn main() {}
