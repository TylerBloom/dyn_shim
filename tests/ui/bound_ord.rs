use dyn_shim::dyn_shim;

// There is no honest total order between unrelated concrete types, so `Ord`
// is rejected; the error points at the hand-written alternatives.
#[dyn_shim(Dyn: Ord)]
trait Src {
    fn get(&self) -> i32;
}

fn main() {}
