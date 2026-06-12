use dyn_shim::dyn_shim;

// Recognition is a bare-ident token match, so a path-form `std::clone::Clone`
// is not intercepted. It passes through as a supertrait, making the shim
// non-dyn-compatible; rustc reports it where the `dyn` type is written.
#[dyn_shim(Dyn: std::clone::Clone)]
trait Src {
    fn get(&self) -> i32;
}

#[derive(Clone)]
struct Widget;
impl Src for Widget {
    fn get(&self) -> i32 {
        1
    }
}

fn main() {
    let b: Box<dyn Dyn> = Box::new(Widget);
    let _ = b.get();
}
