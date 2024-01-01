package firefox

import (
	"errors"
	"fmt"
	"log/slog"
	"path"

	"github.com/spf13/afero"
)

func InstallExtension(fs afero.Fs, srcFile string, profileDir string) (bool, error) {
	fname := path.Base(srcFile)
	extDir := path.Join(profileDir, "extensions")
	f, err := afero.TempFile(fs, extDir, fmt.Sprintf("%s.*", fname))
	if err != nil {
		return false, err
	}

	if linker, ok := fs.(afero.Symlinker); ok {
		to := path.Join(extDir, path.Base(srcFile))
		// Readlink returns the destination of the named symbolic link
		if src, err := linker.ReadlinkIfPossible(to); err == nil {
			if src == srcFile {
				slog.Debug("Firefox extension symlink already up-to-date")
				return false, nil
			}
		}

		tmpName := f.Name()
		_ = f.Close()
		_ = fs.Remove(tmpName)
		slog.Debug("Creating symlink", "dest", tmpName)
		if err := linker.SymlinkIfPossible(srcFile, tmpName); err != nil {
			return false, err
		}
		_ = fs.Remove(to)
		slog.Info("Installing Firefox extension", "to", to)
		if err := fs.Rename(tmpName, to); err != nil {
			return false, err
		}
		return true, nil
	}
	return false, errors.New("filesystem does not support symlinks")
}
