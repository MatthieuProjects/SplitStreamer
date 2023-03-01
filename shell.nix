# This defines a function taking `pkgs` as parameter, and uses
# `nixpkgs` by default if no argument is passed to it.
{ pkgs ? import <nixpkgs> {} }:
	# This avoid typings `pkgs.` before each package name.
	with pkgs;

# Defines a shell.
mkShell {
	# Sets the build inputs, i.e. what will be available in our
	# local environment.
	buildInputs = [
		cargo
		gcc
		go
		gnumake
		protobuf
		rustc
		zlib
		mdbook
		pkg-config
		gst_all_1.gst-plugins-ugly
		gst_all_1.gst-plugins-good
		gst_all_1.gst-plugins-bad
		gst_all_1.gst-plugins-base
		gtk4
		gtk3
		atkmm
 		rustfmt
    		rust-analyzer
    		clippy	
	];
	RUST_BACKTRACE = 1;
}
