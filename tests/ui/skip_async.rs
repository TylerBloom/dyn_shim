use dyn_shim::dyn_shim;

#[allow(async_fn_in_trait)]
#[dyn_shim(Dyn)]
trait Src {
    fn keep(&self) -> i32;
    async fn fetch(&self) -> i32; // async: skipped from the shim
}

struct S;
impl Src for S {
    fn keep(&self) -> i32 {
        0
    }
    async fn fetch(&self) -> i32 {
        0
    }
}

fn main() {
    let d: &dyn Dyn = &S;
    // `fetch` is not part of `Dyn`.
    let _ = d.fetch();
}
