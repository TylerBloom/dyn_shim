use dyn_shim::dyn_shim;

// A `where Self: Sized` method is excluded from the vtable, so it is skipped
// from the shim and reached only on the concrete type.
#[dyn_shim(Dyn)]
trait Src {
    fn keep(&self) -> i32;
    fn sized_only(&self) -> i32
    where
        Self: Sized;
}

struct S;
impl Src for S {
    fn keep(&self) -> i32 {
        0
    }
    fn sized_only(&self) -> i32 {
        1
    }
}

fn main() {
    let d: &dyn Dyn = &S;
    // `sized_only` is not part of `Dyn`.
    let _ = d.sized_only();
}
