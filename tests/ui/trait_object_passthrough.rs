use dyn_shim::trait_object;

// Only recognized traits and auto-trait markers are accepted; an arbitrary trait
// has no machinery to bolt onto the trait's objects.
#[trait_object(Iterator)]
trait Foo {
    fn id(&self) -> u32;
}

fn main() {}
