use dyn_shim::trait_object;

// `trait_object` exists to add a recognized capability (`Clone`/`Hash`). An auto
// trait alone selects marker combinations but names no capability to implement.
#[trait_object(Send)]
trait Foo {
    fn id(&self) -> u32;
}

fn main() {}
