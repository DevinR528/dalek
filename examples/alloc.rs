use ralloc::Ralloc;

#[global_allocator]
static GLOBAL: Ralloc = Ralloc { mmap: 0 };

fn main() {
    let mut v = Vec::new();
    // This will allocate memory using the system allocator.
    v.push(1);
    println!("{:?}", v);
}
