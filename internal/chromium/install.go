package chromium

import (
	"encoding/json"
	"log/slog"
	"path"
	"strings"

	"github.com/michaeladler/extension-downloader/internal/manifest"
	"github.com/spf13/afero"
)

func InstallExtension(fs afero.Fs, crxFile string, profileDir string) (bool, error) {
	mf, err := manifest.ReadManifest(fs, crxFile)
	if err != nil {
		slog.Error("Failed to read Chromium manifest", "crxFile", crxFile, "err", err)
		return false, err
	}

	profileExtensions := path.Join(profileDir, "External Extensions")
	_ = fs.MkdirAll(profileExtensions, 0755)

	var oldMf manifestData
	newMf := manifestData{ExternalCrx: crxFile, ExternalVersion: mf.Version}

	manifestPath := path.Join(profileExtensions, strings.Replace(path.Base(crxFile), ".crx", ".json", 1))
	if _, err := fs.Stat(manifestPath); err == nil {
		if b, err := afero.ReadFile(fs, manifestPath); err == nil {
			if err := json.Unmarshal(b, &oldMf); err == nil {
				if oldMf == newMf {
					slog.Debug("Chromium extension already up-to-date", "name", mf.Name, "version", mf.Version)
					return false, nil
				}
			}
		}
	}

	if oldMf.ExternalVersion != "" {
		slog.Info("Updating Chromium extension", "name", mf.Name, "oldVersion", oldMf.ExternalVersion, "newVersion", mf.Version)
	} else {
		slog.Info("Installing new Chromium extension", "name", mf.Name, "version", mf.Version)
	}

	jsonData, _ := json.Marshal(newMf)
	if err := afero.WriteFile(fs, manifestPath, jsonData, 0644); err != nil {
		slog.Error("Failed to write Chromium manifest", "manifestPath", manifestPath, "err", err)
		return false, err
	}
	return true, nil
}
