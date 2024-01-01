package chromium

import (
	"testing"

	"github.com/spf13/afero"
	"github.com/stretchr/testify/assert"
)

func TestDownloadExtension(t *testing.T) {
	fs := afero.NewMemMapFs()

	extDir, _ := afero.TempDir(fs, "", "TestDownloadExtension_ExtDir")
	id := "dbepggeogbaibhgnhhndojpepiihcmeb"

	fname, err := DownloadExtension(fs, id, extDir)
	assert.NoError(t, err)

	fname2, err := DownloadExtension(fs, id, extDir)
	assert.NoError(t, err)

	assert.Equal(t, *fname, *fname2)

	_, err = fs.Stat(*fname)
	assert.NoError(t, err)
}

func TestDownloadExtension_ReadOnly(t *testing.T) {
	fs := afero.NewMemMapFs()

	extDir, _ := afero.TempDir(fs, "", "TestDownloadExtension_ExtDir")
	id := "dbepggeogbaibhgnhhndojpepiihcmeb"

	fname, err := DownloadExtension(afero.NewReadOnlyFs(fs), id, extDir)
	assert.Error(t, err)
	assert.Nil(t, fname)
}
