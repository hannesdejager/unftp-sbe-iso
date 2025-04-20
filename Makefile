pr-prep:
	cargo fmt --all -- --check
	cargo clippy --all-features -- -D warnings
	cargo test --verbose --all --all-features
	cargo test --doc --all-features
	cargo doc --all-features --no-deps
	cargo check --verbose --all --all-features
	cargo check --examples

clean:
	cargo clean

publish:
	cargo publish --verbose

