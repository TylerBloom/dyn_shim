use dyn_shim::trait_object;

// `trait_object(Hash)` needs the `DynHash` carrier as a supertrait. Without it,
// `dyn Foo` would have no vtable entry to hash through, so the attribute rejects
// it up front rather than emitting an impl whose body fails to compile.
#[trait_object(Hash)]
trait Foo {
    fn id(&self) -> u32;
}

fn main() {}
