use dyn_shim::dyn_shim;
trait Src { fn g<U>(&self, x: U); }
dyn_shim! { trait D for Src { fn g<U>(&self, x: U); } }
fn main() {}
