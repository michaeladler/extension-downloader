package firefox

import (
	"path"
	"testing"

	"github.com/spf13/afero"
	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
)

func TestInstallExtension(t *testing.T) {
	fs := afero.NewOsFs()
	tmpDir, _ := afero.TempDir(fs, "", "TestInstallExtension")
	t.Cleanup(func() {
		_ = fs.RemoveAll(tmpDir)
	})

	profileDir := path.Join(tmpDir, "profile")
	_ = fs.MkdirAll(path.Join(profileDir, "extensions"), 0755)

	srcFile := path.Join(tmpDir, "test.xpi")
	err := afero.WriteFile(fs, srcFile, []byte("hello world"), 0644)
	require.NoError(t, err)

	installed, err := InstallExtension(fs, srcFile, profileDir)
	assert.NoError(t, err)
	assert.True(t, installed)

	installed, err = InstallExtension(fs, srcFile, profileDir)
	assert.NoError(t, err)
	assert.False(t, installed)
}

func TestInstallExtension_ReadOnly(t *testing.T) {
	fs := afero.NewReadOnlyFs(afero.NewMemMapFs())
	installed, err := InstallExtension(fs, "test.xpi", "/profile")
	assert.Error(t, err)
	assert.False(t, installed)
}

func TestInstallExtension_NoSymlinks(t *testing.T) {
	fs := afero.NewMemMapFs()
	installed, err := InstallExtension(fs, "test.xpi", "/profile")
	assert.Error(t, err)
	assert.False(t, installed)
}
