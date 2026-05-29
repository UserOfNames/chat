# MyChat
MyChat is a chat client, server, and protocol.
* Written in Rust
* Protobuf-based network protocol
* Secured with TLS
* Async server, client, and UI
* Reusable client backend with multiple UIs
* Ratatui-based TUI provided as an example

# Running
Currently, the project is pre-release. As such, no pre-compiled binaries are
provided.

## Compiling
1. Ensure you have the latest [Rust toolchain](https://rustup.rs/) installed.
    * If you have Nix, the provided `flake.nix` devshell will install an
    appropriate Rust toolchain version.
2. Open a terminal in the project base directory.
    * When running `ls`, this `README.md` should be visible.
3. Run `cargo build --workspace --release`.
4. The binaries will now be available in `./target/release/`.
    * The server binary is named `chat_server`.
    * The client binary is named `ratatui_frontend`.
5. Alternatively, you may run `cargo run --release --bin <BIN NAME>` to run
   the desired binary directly.
   * For the server, run `cargo run --release --bin chat_server`.
   * For the client, run `cargo run --release --bin ratatui_frontend`.

# Usage instructions
## TLS
To run the server and allow clients to connect to it, you must have a working
TLS leaf certificate. This may be signed by an actual CA, or you may act as an
unofficial CA. Using an official CA-signed certificate is highly recommended
for security reasons. The ability to generate your own CA root is provided for
your convenience, but at your own risk.

Assume the command to run the server binary is `./chat_server`:

* **`./chat_server init pki`**: Initialize a self-signed root CA key and
certificate, and automatically sign a leaf certificate with it. This generates
all four TLS files at once. This is the easiest option to get started if you
are not bringing a standard leaf CA.
* **`./chat_server init ca-certs`**: Initialize a self-signed root CA private key and certificate.
* **`./chat_server init server-certs`**: Given existing files for a root CA
private key and certificate, generate a corresponding leaf private key and
certificate.

## Configuring the server
Once you have your TLS certificates, it is possible to start the server.
However, you may wish to change the default server configuration.

Run `./chat_server init config` to create a default configuration file at the
default path (the command will output the path it wrote to). From there,
customize the config file as needed.

## Running the server
You may run the server with `./chat_server run`. Assuming your TLS leaf
certificate and private key are placed in the default location, this should
work as-is. Alternatively, you may override the default paths using
command-line flags. Use `./chat_server run --help` for more details.

The default paths are platform-specific, and may also depend on environment
variables. Running `./chat_server init server-certs --dry-run` will print the
default server certificate paths for your platform, if supported. Otherwise,
you will have to override them.

Typical default paths for popular platforms are listed below:
* **Windows**: 
    * Certificate: `%APPDATA%\UserOfNames\my_chat\data\server\tls\server\certificate.pem`
    * Key: `%APPDATA%\UserOfNames\my_chat\data\server\tls\server\key.pem`
* **MacOS**: 
    * Certificate: `~/Library/Application Support/rs.UserOfNames.my_chat/server/tls/server/certificate.pem`
    * key: `~/Library/Application Support/rs.UserOfNames.my_chat/server/tls/server/key.pem`
* **Linux**
    * Certificate: `~/.local/share/my_chat/server/tls/server/certificate.pem`
    * Key: `~/.local/share/my_chat/server/tls/server/key.pem`

## Connecting the client
To connect to the server, the client must trust the root CA that signed the
server's leaf certificate. If you use a standard CA, this should work out of
the box. Otherwise, you likely need to manually tell the client to trust your
custom root CA.

To do this, use the `additional_root_ca_paths` argument in the client config
file. You can manually specify a config file path with CLI flags, or use the
default path.

The default path depends on the platform and environment. You can find the
default path with `./ratatui_frontend --get-default-config-path`.

Typical default paths for popular platforms are listed below:
* **Windows**: `%APPDATA%\UserOfNames\my_chat\config\client\config.toml`
* **MacOS**: `~/Library/Application Support/rs.UserOfNames.my_chat/client/config.toml`
* **Linux**: `~/.config/my_chat/client/config.toml`
