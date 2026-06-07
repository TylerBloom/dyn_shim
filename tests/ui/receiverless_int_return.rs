use dyn_shim::dyn_shim;
trait Src { fn answer() -> i32; }
dyn_shim! { trait D for Src { fn answer() -> i32; } }
fn main() {}
