{ buildGoModule, lib }:

buildGoModule rec {
  pname = "extension-downloader";
  version = "0.1.0";

  src = ./.;

  vendorHash = null;

  # requires network connectivity
  doCheck = false;

  ldflags = [
    "-w"
    "-X=main.Version=${version}"
    "-X=main.Commit=git"
    "-X=main.Date=1970-01-01T00:00:00+00:00"
  ];

  meta = with lib; {
    description = "Download browser extensions for Firefox and Chromium-based browsers";
    homepage = "https://github.com/michaeladler/extension-downloader";
    license = licenses.asl20;
    maintainers = with maintainers; [ michaeladler ];
  };
}
