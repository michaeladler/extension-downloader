package chromium

import (
	_ "embed"
	"encoding/json"
	"path"
	"testing"

	"github.com/michaeladler/extension-downloader/internal/manifest"
	"github.com/spf13/afero"
	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
)

//go:embed example.zip
var exampleData []byte

func TestInstallExtension_NotFound(t *testing.T) {
	fs := afero.NewMemMapFs()
	installed, err := InstallExtension(fs, "foo.crx", "profile")
	assert.Error(t, err)
	assert.False(t, installed)
}

func TestInstallExtension(t *testing.T) {
	fs := afero.NewMemMapFs()
	crxFile := "test.crx"
	err := afero.WriteFile(fs, crxFile, exampleData, 0644)
	require.NoError(t, err)

	profileDir := "profile"
	profileExtensions := path.Join(profileDir, "External Extensions")
	_ = fs.MkdirAll(profileExtensions, 0755)

	oldManifest := manifestData{
		ExternalCrx:     crxFile,
		ExternalVersion: "0.1.0",
	}
	jsonData, _ := json.Marshal(oldManifest)
	_ = afero.WriteFile(fs, path.Join(profileExtensions, "test.json"), jsonData, 0644)

	installed, err := InstallExtension(fs, crxFile, profileDir)
	assert.NoError(t, err)
	assert.True(t, installed)
}

func TestInstallExtension_SameVer(t *testing.T) {
	fs := afero.NewMemMapFs()
	crxFile := "test.crx"
	err := afero.WriteFile(fs, crxFile, exampleData, 0644)
	require.NoError(t, err)

	profileDir := "profile"
	profileExtensions := path.Join(profileDir, "External Extensions")
	_ = fs.MkdirAll(profileExtensions, 0755)
	mf, _ := manifest.ReadManifest(fs, crxFile)

	oldManifest := manifestData{
		ExternalCrx:     crxFile,
		ExternalVersion: mf.Version,
	}
	jsonData, _ := json.Marshal(oldManifest)
	_ = afero.WriteFile(fs, path.Join(profileExtensions, "test.json"), jsonData, 0644)

	installed, err := InstallExtension(fs, crxFile, profileDir)
	assert.NoError(t, err)
	assert.False(t, installed)
}

func TestInstallExtension_ReadOnly(t *testing.T) {
	fs := afero.NewMemMapFs()
	crxFile := "test.crx"
	err := afero.WriteFile(fs, crxFile, exampleData, 0644)
	require.NoError(t, err)

	profileDir := "profile"
	profileExtensions := path.Join(profileDir, "External Extensions")
	_ = fs.MkdirAll(profileExtensions, 0755)

	installed, err := InstallExtension(afero.NewReadOnlyFs(fs), crxFile, profileDir)
	assert.Error(t, err)
	assert.False(t, installed)
}
