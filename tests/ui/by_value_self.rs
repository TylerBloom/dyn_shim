use dyn_shim::dyn_shim;
trait Src { fn consume(self, s: u8) -> u8; }
dyn_shim! { trait D for Src { fn consume(self, s: u8) -> u8; } }
fn main() {}
