use dyn_shim::dyn_shim;

// The macro cannot classify an arbitrary trait in the bounds list, so a
// non-dyn-compatible one passes through as a supertrait and the shim is not
// dyn-compatible either. rustc reports it where the `dyn` type is written.
trait Factory {
    fn make() -> Self;
}

#[dyn_shim(Dyn: Factory)]
trait Src {
    fn get(&self) -> i32;
}

struct Widget;
impl Factory for Widget {
    fn make() -> Self {
        Widget
    }
}
impl Src for Widget {
    fn get(&self) -> i32 {
        1
    }
}

fn main() {
    let b: Box<dyn Dyn> = Box::new(Widget);
    let _ = b.get();
}
