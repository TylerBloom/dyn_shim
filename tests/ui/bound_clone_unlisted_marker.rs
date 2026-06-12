use dyn_shim::dyn_shim;

// Marker combinations are generated only for auto traits listed in the
// bounds. `Sticker` lists none, so `Box<dyn DynSticker>` is `Clone` while
// `Box<dyn DynSticker + Send>` is not.
#[dyn_shim(DynSticker: Clone)]
trait Sticker {
    fn label(&self) -> String;
}

#[derive(Clone)]
struct Tag;
impl Sticker for Tag {
    fn label(&self) -> String {
        "tag".into()
    }
}

fn assert_clone<T: Clone>(_: &T) {}

fn main() {
    let plain: Box<dyn DynSticker> = Box::new(Tag);
    assert_clone(&plain);
    let send: Box<dyn DynSticker + Send> = Box::new(Tag);
    assert_clone(&send);
}
