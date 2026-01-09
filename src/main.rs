fn main() {
    let n = uring_fast_read::read_one_file("Cargo.toml").unwrap();
    println!("Read {} bytes", n);
}
