use dyn_shim::dyn_shim;

// `#[dyn_shim(...)]` helper attributes are only supported on methods; on any
// other trait item the macro rejects them directly.
#[dyn_shim(Dyn)]
trait Src {
    #[dyn_shim(skip)]
    const LIMIT: usize;
    fn keep(&self) -> i32;
}

fn main() {}
