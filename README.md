[![ci](https://github.com/michaeladler/extension-downloader/actions/workflows/ci.yml/badge.svg)](https://github.com/michaeladler/extension-downloader/actions/workflows/ci.yml)
[![codecov](https://codecov.io/gh/michaeladler/extension-downloader/graph/badge.svg?token=zMbpcaIgxc)](https://codecov.io/gh/michaeladler/extension-downloader)

# extension-downloader

`extension-downloader` is a tool to **download browser extensions** for Firefox and Chromium-based browsers.
It works on Linux, macOS, and Windows systems.
A typical use-case is to maintain and deploy browser extensions across multiple systems.

## Installation

To install `extension-downloader`, follow these steps:

1. Build and install `extension-downloader` using Rust's cargo:

```bash
cargo install --git https://github.com/michaeladler/extension-downloader.git
```

2. Copy the example configuration file to your user configuration directory:

```bash
mkdir -p ~/.config/extension-downloader
cp example/config.toml ~/.config/extension-downloader/config.toml
```

3. Adjust the `config.toml` to your needs.

## Usage

After configuring `config.toml`, you can run the tool using:

```bash
./extension-downloader
```

The downloader will fetch and install the extensions specified in the configuration file.
See `./extension-downloader --help` for available CLI options.

## Configuration

To configure `extension-downloader`, edit the `config.toml` file to specify which extensions you'd like to download.
The configuration options include specifying the extension IDs (or names), the destination directory, and browser type.

Example config snippet:

```toml
[[extensions]]
# browser can be one of: "firefox", "chromium"
browser = "firefox"
# tilde (~) is expanded, anything else not
profile = "~/.mozilla/firefox/default"
# firefox extensions are referenced by name
names = ["ublock-origin"]

[[extensions]]
browser = "chromium"
profile = "~/.config/chromium"
# chromium extensions are referenced by their ID
# which can be obtained from the URL in the Chrome web store
names = [
    "cjpalhdlnbpafiamejdnhcphjbkeiagm", # ublock-origin
]

# Microsoft Edge on Windows
[[extensions]]
browser = "chromium"
profile = "Microsoft\\Edge"
names = [
    "cjpalhdlnbpafiamejdnhcphjbkeiagm", # ublock-origin
]
```

**Note**: Each extension is downloaded only **once** and then **shared** across compatible browsers.

## Contributing

If you'd like to contribute to `extension-downloader`, please fork the repository and create a pull request, or open an issue for discussion regarding changes or features you'd like to add.

## License

`extension-downloader` is made available under the [Apache-2.0 License](LICENSE).
