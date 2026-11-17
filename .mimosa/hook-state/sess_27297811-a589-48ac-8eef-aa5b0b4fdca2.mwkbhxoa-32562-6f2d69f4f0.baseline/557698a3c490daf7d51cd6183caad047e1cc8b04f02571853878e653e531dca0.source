fn main() {
    // sqlx::migrate! embeds the directory at compile time. Cargo otherwise
    // does not notice newly added migration files when no Rust source changes.
    println!("cargo:rerun-if-changed=../../../sql/postgresql");
}
