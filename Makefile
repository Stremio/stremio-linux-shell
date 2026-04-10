.PHONY: bundle release clean

bundle: release
	cp -f target/release/stremio-linux-shell target/release/bundle/

release:
	cargo build --release

clean:
	cargo clean
