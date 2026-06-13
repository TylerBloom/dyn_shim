use dyn_shim::dyn_shim;

// `reflexive = bare` generates `impl Src for dyn Dyn`, where `Self` is the
// unsized `dyn Dyn`. A by-value `self` receiver cannot be expressed there (it
// would take the unsized type by value), so the macro rejects it up front and
// points at `reflexive = boxed`, where the receiver becomes `Box<dyn Dyn>`.
#[dyn_shim(Dyn, reflexive = bare)]
trait Src {
    fn get(&self) -> i32;
    fn consume(self) -> i32;
}

fn main() {}
