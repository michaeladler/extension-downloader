{
  lib,
  rustPlatform,
  pkg-config,
  openssl,
  cacert,
}:

rustPlatform.buildRustPackage {
  pname = "extension-downloader";
  version = "0.2.0";

  src = ./.;

  nativeBuildInputs = [ pkg-config ];

  buildInputs = [ openssl ];

  checkInputs = [ cacert ];

  cargoLock = {
    lockFile = ./Cargo.lock;
  };

  meta = with lib; {
    description = "Download browser extensions for Firefox and Chromium-based browsers";
    homepage = "https://github.com/michaeladler/extension-downloader";
    license = licenses.asl20;
    maintainers = with maintainers; [ michaeladler ];
  };
}
