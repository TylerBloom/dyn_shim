use dyn_shim::dyn_shim;

// A recognized `Hash` bound requires implementors of the source trait to be
// `Hash` before they receive the shim; a non-`Hash` implementor cannot be
// used as the shim's trait object.
#[dyn_shim(Dyn: Hash)]
trait Src {
    fn get(&self) -> i32;
}

struct NotHash;
impl Src for NotHash {
    fn get(&self) -> i32 {
        1
    }
}

fn main() {
    let b: Box<dyn Dyn> = Box::new(NotHash);
    let _ = b.get();
}
