package config

import (
	"testing"

	"github.com/spf13/afero"
	"github.com/stretchr/testify/assert"
)

func TestLoadConfig(t *testing.T) {
	exampleConfig := `
[[extensions]]
browser = "firefox"
profile = "~/.mozilla/firefox/default"
names = ["ublock-origin"]

[[extensions]]
browser = "chromium"
profile = "~/.config/chromium"
names = [
    "cjpalhdlnbpafiamejdnhcphjbkeiagm", # ublock-origin
]`

	fs := afero.NewMemMapFs()
	_ = afero.WriteFile(fs, "config.toml", []byte(exampleConfig), 0644)
	cfg, err := LoadConfig(fs, "config.toml")
	assert.NoError(t, err)
	assert.NotNil(t, cfg)
	assert.Len(t, cfg.Extensions, 2)
	assert.Len(t, cfg.Extensions[0].Names, 1)
	assert.Len(t, cfg.Extensions[1].Names, 1)
}

func TestLoadConfig_InvalidBrowser(t *testing.T) {
	exampleConfig := `
[[extensions]]
browser = "foo"
profile = "~/.mozilla/firefox/default"
names = ["ublock-origin"]
`

	fs := afero.NewMemMapFs()
	_ = afero.WriteFile(fs, "config.toml", []byte(exampleConfig), 0644)
	cfg, err := LoadConfig(fs, "config.toml")
	assert.Error(t, err)
	assert.ErrorContains(t, err, "unsupported browser")
	assert.Nil(t, cfg)
}

func TestLoadConfig_NotFound(t *testing.T) {
	fs := afero.NewMemMapFs()
	cfg, err := LoadConfig(fs, "config.toml")
	assert.Nil(t, cfg)
	assert.Error(t, err)
}

func TestLoadConfig_InvalidToml(t *testing.T) {
	fs := afero.NewMemMapFs()
	_ = afero.WriteFile(fs, "config.toml", []byte("<html>"), 0644)
	cfg, err := LoadConfig(fs, "config.toml")
	assert.Nil(t, cfg)
	assert.Error(t, err)
}
