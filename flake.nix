{
  description = "Svelte 5 + Tauri v2 development environment";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, flake-utils }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs { inherit system; };

        # System dependencies for Tauri v2 on Linux
        libraries = with pkgs; [
          webkitgtk_4_1
          gtk3
          cairo
          gdk-pixbuf
          glib
          dbus
          openssl
          librsvg
          libsoup_3
        ];

        packages = with pkgs; [
          curl
          wget
          pkg-config
          dbus
          openssl
          librsvg
          webkitgtk_4_1
          glib
          gtk3
          libsoup_3
          # Rust & Frontend
          rustc
          cargo
          nodejs_20 # Svelte 5 works great with Node 20+
          bun
        ];
      in
      {
        devShells.default = pkgs.mkShell {
          buildInputs = packages;

          # Tauri needs to know where these libraries are during compilation
          shellHook = ''
            export LD_LIBRARY_PATH=${pkgs.lib.makeLibraryPath libraries}:$LD_LIBRARY_PATH
            export XDG_DATA_DIRS=$GSETTINGS_SCHEMAS_PATH:$XDG_DATA_DIRS
            echo "🚀 Tauri v2 + Svelte 5 Dev Environment Loaded"
            echo "Rust: $(rustc --version)"
            echo "Node: $(node --version)"
          '';
        };
      });
}
