[![wakatime](https://wakatime.com/badge/github/angeloanan/bmap-server.svg)](https://wakatime.com/badge/github/angeloanan/bmap-server)

# BlueMap Server

A simple external web server for [BlueMap](https://bluemap.bluecolored.de/).

The motivation of this project is to provide a simple but robust solution on serving BlueMap's live data to multiple clients.

## Usage

Download the latest release from the [releases page](https://github.com/angeloanan/bmap-server/releases).

Allow the app to be runnable by `chmod +x bmap-server`.

Run the app with the path to your BlueMap data directory as an argument:

```sh
$ ./bmap-server /path/to/bluemap/data
```

You can also use the `--help` flag to get a list of all available options:

<details>
<summary>Click to expand</summary>

```
$ bmap-server --help
Usage: bmap-server [OPTIONS] <BLUEMAP_DIR>

Arguments:
  <BLUEMAP_DIR>  Path to Bluemap's data directory

Options:
      --host <HOST>                  Host to listen [default: 0.0.0.0]
  -p, --port <PORT>                  Port to listen [default: 31283]
      --bluemap-host <BLUEMAP_HOST>  Bluemap's Live Server host [default: 127.0.0.1]
      --bluemap-port <BLUEMAP_PORT>  Bluemap's Live Server port [default: 8100]
      --tls-cert <TLS_CERT>          TLS certificate file - If not provided, server will run without TLS
      --tls-key <TLS_KEY>            TLS key file - If not provided, server will run without TLS
  -h, --help                         Print help
  -V, --version                      Print version
```

</details>

## Building

This project uses [Rust](https://www.rust-lang.org/) and [Cargo](https://doc.rust-lang.org/cargo/).

You do not need to have OpenSSL installed to build the project as the project uses the [rustls](https://github.com/rustls/rustls) crate to provide TLS support.

To build the project, clone the repository and run `cargo build`.

## Contributing

Contributions are welcome, though not expected and not guaranteed to be merged; this is a personal project after all.

Feel free to fork and adapt this project to your needs.

