use dyn_shim::dyn_shim;

// The only supported helper argument on a method is `skip`; anything else
// (here a typo of it) is an error instead of being silently ignored.
#[dyn_shim(Dyn)]
trait Src {
    #[dyn_shim(skpi)]
    fn keep(&self) -> i32;
}

fn main() {}
