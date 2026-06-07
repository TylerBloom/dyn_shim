use dyn_shim::dyn_shim;
trait Src { fn make() -> Self where Self: Sized; }
dyn_shim! { trait D for Src { fn make() -> Self; } }
fn main() {}
