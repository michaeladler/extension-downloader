package netutil

import (
	"errors"
	"io"
	"log/slog"
	"net/http"

	"github.com/spf13/afero"
)

func DownloadFile(fs afero.Fs, dest string, url string) error {
	slog.Debug("Downloading file", "url", url)
	resp, err := http.Get(url)
	if err != nil {
		slog.Error("Failed to GET url", "url", url, "err", err)
		return err
	}
	defer resp.Body.Close()
	if resp.StatusCode != http.StatusOK {
		slog.Error("Bad status code", "status", resp.Status)
		return errors.New("bad status code")
	}

	f, err := fs.Create(dest)
	if err != nil {
		slog.Error("Failed to create destination file", "dest", dest, "err", err)
		return err
	}
	defer f.Close()

	if _, err := io.Copy(f, resp.Body); err != nil {
		slog.Error("Failed to copy response body", "err", err)
		return err
	}
	return nil
}
