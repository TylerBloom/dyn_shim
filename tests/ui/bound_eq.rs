use dyn_shim::dyn_shim;

// Equality between trait objects needs an `Any` downcast the macro does not
// generate yet; rejected with a targeted error rather than letting `&Self`
// in `eq` break the shim.
#[dyn_shim(Dyn: Eq)]
trait Src {
    fn get(&self) -> i32;
}

fn main() {}
