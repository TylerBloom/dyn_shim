use dyn_shim::dyn_shim_recognized;

// Only `Clone` and `Hash` have a built-in dyn-compatible erasure. An arbitrary
// trait is rejected rather than silently producing a broken shim.
#[dyn_shim_recognized(Debug)]
trait DynDbg {}

fn main() {}
