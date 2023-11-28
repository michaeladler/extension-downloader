package netutil

import (
	"crypto/sha256"
	"fmt"
	"io"
	"testing"

	"github.com/spf13/afero"
	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
)

func TestDownloadFile(t *testing.T) {
	fs := afero.NewMemMapFs()
	err := DownloadFile(fs, "LICENSE", "https://www.apache.org/licenses/LICENSE-2.0.txt")
	assert.NoError(t, err)

	f, err := fs.Open("LICENSE")
	require.NoError(t, err)
	t.Cleanup(func() {
		f.Close()
	})
	hasher := sha256.New()
	_, err = io.Copy(hasher, f)
	require.NoError(t, err)
	checksum := fmt.Sprintf("%x", hasher.Sum(nil))
	assert.Equal(t, "cfc7749b96f63bd31c3c42b5c471bf756814053e847c10f3eb003417bc523d30", string(checksum))
}

func TestDownloadFile_InvalidUrl(t *testing.T) {
	fs := afero.NewMemMapFs()
	err := DownloadFile(fs, "LICENSE", "http://10.0.0.1:65000")
	assert.Error(t, err)
}

func TestDownloadFile_StatusCode(t *testing.T) {
	fs := afero.NewMemMapFs()
	err := DownloadFile(fs, "index.html", "http://www.google.de/123")
	assert.Error(t, err)
}

func TestDownloadFile_WriteFail(t *testing.T) {
	fs := afero.NewReadOnlyFs(afero.NewMemMapFs())
	err := DownloadFile(fs, "LICENSE", "https://www.apache.org/licenses/LICENSE-2.0.txt")
	assert.Error(t, err)
}
