package manifest

import (
	_ "embed"
	"testing"

	"github.com/spf13/afero"
	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
)

//go:embed example.zip
var exampleData []byte

//go:embed hello.zip
var helloData []byte

func TestReadManifest(t *testing.T) {
	fs := afero.NewMemMapFs()
	err := afero.WriteFile(fs, "example.zip", exampleData, 0644)
	require.NoError(t, err)

	mf, err := ReadManifest(fs, "example.zip")
	require.NoError(t, err)
	assert.Equal(t, "1.53.0", mf.Version)
	assert.Equal(t, "uBlock Origin", mf.Name)
}

func TestReadManifest_NotFound(t *testing.T) {
	fs := afero.NewMemMapFs()
	mf, err := ReadManifest(fs, "example.zip")
	require.Error(t, err)
	assert.Nil(t, mf)
}

func TestReadManifest_NoZip(t *testing.T) {
	fs := afero.NewMemMapFs()
	_ = afero.WriteFile(fs, "example.txt", []byte("<html>"), 0644)
	mf, err := ReadManifest(fs, "example.txt")
	require.Error(t, err)
	assert.Nil(t, mf)
}

func TestReadManifest_NoNeedle(t *testing.T) {
	fs := afero.NewMemMapFs()
	_ = afero.WriteFile(fs, "test.zip", helloData, 0644)
	mf, err := ReadManifest(fs, "test.zip")
	require.Error(t, err)
	assert.Nil(t, mf)
}
