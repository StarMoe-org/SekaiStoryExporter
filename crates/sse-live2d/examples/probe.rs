//! `cargo run -p sse-live2d --example probe -- <model.moc3>`: loads a moc3 through Core.
fn main() {
    let path = std::env::args().nth(1).expect("moc3 path");
    let bytes = std::fs::read(&path).expect("read");
    println!("core {:?}", sse_live2d::core_version());
    let moc = sse_live2d::Moc::new(&bytes).expect("moc");
    let mut model = sse_live2d::Model::new(moc.clone()).expect("model");
    println!(
        "moc version {} params {} parts {} drawables {} canvas {:?}",
        moc.moc_version,
        model.parameter_ids.len(),
        model.part_ids.len(),
        model.drawables.len(),
        model.canvas
    );
    let d = model.parameter_default.clone();
    model.update(&d, None);
    let frames = model.drawable_frames();
    let (mut lo, mut hi) = ([f32::MAX; 2], [f32::MIN; 2]);
    for f in &frames {
        for p in &f.positions {
            lo = [lo[0].min(p[0]), lo[1].min(p[1])];
            hi = [hi[0].max(p[0]), hi[1].max(p[1])];
        }
    }
    println!(
        "bounds {lo:?} {hi:?}; masked {}",
        model
            .drawables
            .iter()
            .filter(|d| !d.masks.is_empty())
            .count()
    );
    println!("{:?}", &model.parameter_ids[..12]);
}
