package manifest

import (
	"archive/zip"
	"bytes"
	"encoding/json"
	"errors"
	"io"

	"github.com/spf13/afero"
)

type Manifest struct {
	Name    string `json:"name"`
	Version string `json:"version"`
}

func ReadManifest(fs afero.Fs, fname string) (*Manifest, error) {
	f, err := fs.Open(fname)
	if err != nil {
		return nil, err
	}
	defer f.Close()
	fi, err := f.Stat()
	if err != nil {
		return nil, err
	}
	r, err := zip.NewReader(f, fi.Size())
	if err != nil {
		return nil, err
	}

	// search our needle in the haystack
	needle := "manifest.json"
	for _, f := range r.File {
		if f.Name == needle {
			rc, err := f.Open()
			if err != nil {
				return nil, err
			}
			defer rc.Close()

			var buf bytes.Buffer
			_, err = io.Copy(&buf, rc)
			if err != nil {
				return nil, err
			}

			result := new(Manifest)
			if err := json.Unmarshal(buf.Bytes(), result); err != nil {
				return nil, err
			}
			return result, nil
		}
	}
	return nil, errors.New("manifest.json not found in file")
}
