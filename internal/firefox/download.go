package firefox

import (
	"encoding/json"
	"fmt"
	"io"
	"log/slog"
	"net/http"
	"path/filepath"

	"github.com/michaeladler/extension-downloader/internal/manifest"
	"github.com/michaeladler/extension-downloader/internal/netutil"
	"github.com/spf13/afero"
)

type extension struct {
	Guid           string   `json:"guid"`
	CurrentVersion metadata `json:"current_version"`
}

type metadata struct {
	Version string `json:"version"`
	Files   []file `json:"files"`
}

type file struct {
	Url  string `json:"url"`
	Hash string `json:"hash"`
}

func DownloadExtension(fs afero.Fs, name string, destDir string) (*string, error) {
	slog.Debug("Checking Firefox upstream manifest", "name", name)
	url := fmt.Sprintf("https://services.addons.mozilla.org/api/v4/addons/addon/%s/", name)
	response, err := http.Get(url)
	if err != nil {
		slog.Error("Failed to get url", "url", url, "err", err)
		return nil, err
	}
	defer response.Body.Close()

	body, err := io.ReadAll(response.Body)
	if err != nil {
		slog.Error("Failed to read response body", "name", name, "err", err)
		return nil, err
	}

	var ext extension
	if err = json.Unmarshal(body, &ext); err != nil {
		slog.Error("Failed to unmarshal JSON", "name", name, "err", err)
		return nil, err
	}
	version := ext.CurrentVersion.Version

	dest := filepath.Join(destDir, fmt.Sprintf("%s.xpi", ext.Guid))
	if _, err := fs.Stat(dest); err == nil {
		// check if file is up-to-date
		if oldMf, err := manifest.ReadManifest(fs, dest); err == nil {
			oldVersion := oldMf.Version
			if oldVersion == version {
				slog.Debug("Firefox extension already up-to-date", "name", name, "version", version)
				return &dest, nil
			} else {
				slog.Info("Updating Firefox extension", "name", name, "oldVersion", oldVersion, "version", version)
			}
		}
	} else {
		slog.Info("Installing new Firefox extension", "name", name, "version", version)
	}

	slog.Debug("Downloading Firefox extension", "name", name)
	if err := netutil.DownloadFile(fs, dest, ext.CurrentVersion.Files[0].Url); err != nil {
		return nil, err
	}
	return &dest, nil
}
