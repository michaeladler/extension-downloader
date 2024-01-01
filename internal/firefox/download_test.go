package firefox

import (
	"path"
	"testing"

	"github.com/spf13/afero"
	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
)

func TestDownloadExtension(t *testing.T) {
	fs := afero.NewMemMapFs()
	destDir, _ := afero.TempDir(fs, "", "TestDownloadExtension")

	fname, err := DownloadExtension(fs, "vimium-ff", destDir)
	require.NoError(t, err)
	assert.Equal(t, path.Join(destDir, "{d7742d87-e61d-4b78-b8a1-b469842139fa}.xpi"), *fname)

	fname2, err := DownloadExtension(fs, "vimium-ff", destDir)
	require.NoError(t, err)
	assert.Equal(t, *fname, *fname2)
}

func TestInstallExtension_InvalidName(t *testing.T) {
	name := []byte{0x01}
	fs := afero.NewMemMapFs()
	destDir, _ := afero.TempDir(fs, "", "TestDownloadExtension")

	fname, err := DownloadExtension(fs, string(name), destDir)
	assert.Error(t, err)
	assert.Nil(t, fname)
}

func TestInstallExtension_NotFound(t *testing.T) {
	name := []byte{0x01}
	fs := afero.NewMemMapFs()
	fname, err := DownloadExtension(fs, string(name), "thisdoesnotexist")
	assert.Error(t, err)
	assert.Nil(t, fname)
}
