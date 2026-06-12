use dyn_shim::dyn_shim;

// A recognized `Clone` bound requires implementors of the source trait to be
// `Clone` before they receive the shim; a non-`Clone` implementor cannot be
// used as the shim's trait object.
#[dyn_shim(Dyn: Clone)]
trait Src {
    fn get(&self) -> i32;
}

struct NotClone;
impl Src for NotClone {
    fn get(&self) -> i32 {
        1
    }
}

fn main() {
    let b: Box<dyn Dyn> = Box::new(NotClone);
    let _ = b.get();
}
