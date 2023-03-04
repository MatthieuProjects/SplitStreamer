target/release/confgen:
	cargo build --release --bin confgen
client.json: target/release/confgen
	target/release/confgen --screen-width 1600 --screen-height 1280 -c 3 -l 2 -p 5000 -a 224.1.1.1 > client.json

clean:
	rm target/release/confgen client.json

.PHONY: clean
