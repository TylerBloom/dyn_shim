use dyn_shim::dyn_shim_foreign;

// The attribute's argument is the foreign trait's path. Omitting it is a direct
// parse error, not a confusing rustc error later.
#[dyn_shim_foreign]
trait DynSink {
    fn write(&mut self, line: &str);
}

fn main() {}
