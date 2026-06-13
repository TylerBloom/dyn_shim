use dyn_shim::dyn_shim_recognized;

// The shim's contents are generated from the recognized trait, so it must not
// declare methods of its own.
#[dyn_shim_recognized(Clone)]
trait DynCloneable {
    fn extra(&self) -> i32;
}

fn main() {}
