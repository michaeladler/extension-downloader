package chromium

import (
	"fmt"
	"log/slog"
	"path"

	"github.com/michaeladler/extension-downloader/internal/netutil"
	"github.com/spf13/afero"
)

type manifestData struct {
	ExternalCrx     string `json:"external_crx"`
	ExternalVersion string `json:"external_version"`
}

func DownloadExtension(fs afero.Fs, id string, extDir string) (*string, error) {
	prefix := "https://clients2.google.com/service/update2/crx?response=redirect&os=linux&arch=x64&os_arch=x86_64&nacl_arch=x86-64&prod=chromium&prodchannel=unknown&prodversion=91.0.4442.4&lang=en-US&acceptformat=crx2,crx3&x=id%3D"
	src := fmt.Sprintf("%s%s%s", prefix, id, "%26installsource%3Dondemand%26uc")
	destFile := path.Join(extDir, fmt.Sprintf("%s.crx", id))
	slog.Debug("Downloading Chromium extension", "id", id)
	if err := netutil.DownloadFile(fs, destFile, src); err != nil {
		slog.Error("Failed to download Chromium extension", "id", id, "err", err)
		return nil, err
	}
	return &destFile, nil
}
