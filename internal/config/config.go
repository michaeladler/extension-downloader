package config

import (
	"github.com/BurntSushi/toml"
	"github.com/michaeladler/extension-downloader/internal/pathutil"
	"github.com/spf13/afero"
)

type AppConfig struct {
	Extensions []ExtensionConfig `toml:"extensions"`
}

type ExtensionConfig struct {
	Browser string   `toml:"browser"`
	Profile string   `toml:"profile"`
	Names   []string `toml:"names"`
}

func LoadConfig(fs afero.Fs, fname string) (*AppConfig, error) {
	b, err := afero.ReadFile(fs, fname)
	if err != nil {
		return nil, err
	}

	conf := new(AppConfig)
	if _, err := toml.Decode(string(b), conf); err != nil {
		return nil, err
	}
	// normalize paths
	for i := range conf.Extensions {
		conf.Extensions[i].Profile = pathutil.ExpandUser(conf.Extensions[i].Profile)
	}
	return conf, nil
}
