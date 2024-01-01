package main

import (
	"path"
	"testing"

	"github.com/michaeladler/extension-downloader/internal/pathutil"
	"github.com/spf13/afero"
	"github.com/stretchr/testify/assert"
)

const exampleConfig string = `
[[extensions]]
browser = "firefox"
profile = "/mozilla/firefox/default"
names = ["ublock-origin"]

[[extensions]]
browser = "chromium"
profile = "/config/chromium"
names = [
    "cjpalhdlnbpafiamejdnhcphjbkeiagm", # ublock-origin
]`

func TestMain_NoConfig(t *testing.T) {
	fs := afero.NewMemMapFs()
	result := _main(fs)
	assert.Equal(t, 1, result)
}

func TestMain(t *testing.T) {
	rootFS := afero.NewOsFs()
	tmpDir, _ := afero.TempDir(rootFS, "", "TestMain")
	t.Cleanup(func() {
		_ = rootFS.RemoveAll(tmpDir)
	})

	// restrict all operations to a given path within an Fs
	fs := afero.NewBasePathFs(rootFS, tmpDir)
	_ = fs.MkdirAll("/mozilla/firefox/default/extensions", 0755)

	cfgName := pathutil.ExpandUser("~/.config/extension-downloader/config.toml")
	_ = fs.MkdirAll(path.Dir(cfgName), 0755)
	_ = afero.WriteFile(fs, cfgName, []byte(exampleConfig), 0644)

	result := _main(fs)
	assert.Equal(t, 0, result)
}

func TestMain_VersionFlag(t *testing.T) {
	fs := afero.NewReadOnlyFs(afero.NewMemMapFs())
	versionFlag = true
	t.Cleanup(func() {
		versionFlag = false
	})
	result := _main(fs)
	assert.Equal(t, 0, result)
}

func TestMain_InvalidLogLeel(t *testing.T) {
	fs := afero.NewReadOnlyFs(afero.NewMemMapFs())
	logLevelFlag = "foo"
	t.Cleanup(func() {
		logLevelFlag = ""
	})
	result := _main(fs)
	assert.Equal(t, 1, result)
}

func TestMain_ReadOnlyFs(t *testing.T) {
	fs := afero.NewMemMapFs()
	cfgName := pathutil.ExpandUser("~/.config/extension-downloader/config.toml")
	_ = fs.MkdirAll(path.Dir(cfgName), 0755)
	_ = afero.WriteFile(fs, cfgName, []byte(exampleConfig), 0644)

	result := _main(afero.NewReadOnlyFs(fs))
	assert.Equal(t, 2, result)
}
