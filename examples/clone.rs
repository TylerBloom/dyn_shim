//! Cloneable boxed trait objects with `#[dyn_shim]`.
//!
//! The `Shape` trait below wants cloneable implementors, but `Clone` cannot
//! be one of its supertraits without giving up dyn-compatibility (`clone`
//! returns `Self`). Instead, a `Clone` in the attribute's bounds is
//! recognized and handled specially: the generated `DynShape` shim carries
//! hidden machinery so `Box<dyn DynShape>` implements `Clone`, and only
//! `Clone` implementors of `Shape` receive the shim.
//!
//! Coming from the `dyn_clone` crate? `examples/migrate_dyn_clone.rs` walks
//! through the conversion step by step.
//!
//! Run with: `cargo run --example clone`

use dyn_shim::dyn_shim;

#[dyn_shim(DynShape: Clone)]
trait Shape {
    fn name(&self) -> &'static str;
    fn area(&self) -> f64;
    fn scale(&mut self, factor: f64);
}

#[derive(Clone)]
struct Circle {
    radius: f64,
}

impl Shape for Circle {
    fn name(&self) -> &'static str {
        "circle"
    }
    fn area(&self) -> f64 {
        std::f64::consts::PI * self.radius * self.radius
    }
    fn scale(&mut self, factor: f64) {
        self.radius *= factor;
    }
}

#[derive(Clone)]
struct Rect {
    w: f64,
    h: f64,
}

impl Shape for Rect {
    fn name(&self) -> &'static str {
        "rect"
    }
    fn area(&self) -> f64 {
        self.w * self.h
    }
    fn scale(&mut self, factor: f64) {
        self.w *= factor;
        self.h *= factor;
    }
}

fn main() {
    let shapes: Vec<Box<dyn DynShape>> = vec![
        Box::new(Circle { radius: 1.0 }),
        Box::new(Rect { w: 2.0, h: 3.0 }),
    ];

    // Box<dyn DynShape> is Clone, so the whole Vec clones. Mutating the
    // copies leaves the originals untouched.
    let mut grown = shapes.clone();
    for shape in grown.iter_mut() {
        shape.scale(2.0);
    }

    for (original, copy) in shapes.iter().zip(&grown) {
        println!(
            "{}: area {:.2}, scaled clone area {:.2}",
            original.name(),
            original.area(),
            copy.area()
        );
    }
}
