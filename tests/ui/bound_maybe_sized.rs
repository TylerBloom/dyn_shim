use dyn_shim::dyn_shim;

// A modified bound is not recognized: the `?` keeps this from matching the
// `Sized` rejection, so it passes through to the generated trait, where
// rustc rejects `?Trait` in supertrait position.
#[dyn_shim(Dyn: ?Sized)]
trait Src {
    fn get(&self) -> i32;
}

fn main() {}
